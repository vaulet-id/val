// The catalogue a VAL screen is drawn from.
//
// This is not a picture of what a VAL screen would look like. It is the
// catalogue: `button` is a Flutter button, `tabs` is a Flutter TabBar, and the
// text is laid out by the same engine that lays it out on a phone. A facsimile
// in HTML would have agreed with itself and been wrong about wrapping, about
// Thai line breaking, and about every metric that matters.
//
// The application supplies structure and keys. Everything visual — spacing,
// colour, type, the empty state, which tab is open — belongs here, which is the
// division the specification draws and the reason a screen can be trusted.
//
// **One implementation, two hosts.** The playground draws with it and so does
// the wallet. Two of these would agree for as long as somebody kept comparing
// them by eye, and a screen that draws differently on a phone than it did where
// it was written is a screen nobody can be shown before it ships.

import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';

/// Draw a package's screens.
///
/// **The host owns the stack and the forms.** An application declares where a
/// press goes and never how the stack behaves, which is why there is no push,
/// pop or replace for it to get wrong — and what somebody typed survives going
/// somewhere and coming back, because the form is kept above the screen rather
/// than inside it.
///
/// What leaves here is a press the application declared: `onTap` names an
/// action, and the host runs it. Moving between screens never reaches the
/// application at all.
class ValApp extends StatefulWidget {
  const ValApp({
    super.key,
    required this.incoming,
    required this.onTap,
    this.onScreen,
    this.cards,
    this.chooser,
  });

  /// The screens as the compiler resolved them, the signed text bundle, the
  /// locale and whether it is dark.
  final Incoming incoming;

  /// A press on something that names an action: the action, and what the form
  /// on that screen holds.
  final void Function(String action, Map<String, Object?> input) onTap;

  /// A screen that takes parameters cannot have been resolved ahead of time —
  /// its content depends on the row that opened it — so the host is asked for
  /// it with what the press handed over.
  final void Function(String screen, Map<String, Object?> args)? onScreen;

  /// **How this host draws one of its own credentials** — `credentialCard(of:
  /// EmployeeBadge)`, and `wallet.idCard` where a package has stopped being
  /// portable.
  ///
  /// The application names which card and never draws it: it holds a check or
  /// a read, and either way the face of a card — the issuer's colour, the
  /// issuer's current word about it, whatever a withdrawal looks like this
  /// month — belongs to the host that issued the design. One place to change
  /// it, and every application changes with it.
  ///
  /// Null draws a plain stand-in, which is what a playground with no wallet
  /// behind it has to show.
  final Widget Function(BuildContext context, String credential)? cards;

  /// How this host asks somebody to choose one of a few things.
  ///
  /// **A wallet has one drawer**, and a menu that drops out of a field is not
  /// it: the same question asked in a Micro App and asked in the wallet's own
  /// forms should be the same object on the screen. The host is handed the
  /// title and the options and hands back what was picked, or null.
  ///
  /// Null draws a plain sheet of this renderer's own, which is what a
  /// playground with no wallet behind it has to show.
  final Future<String?> Function(BuildContext context, String title, List<String> options)?
      chooser;

  @override
  State<ValApp> createState() => _ValAppState();
}

class _ValAppState extends State<ValApp> {
  /// Which screens have been opened, deepest last.
  final List<String> _stack = [];

  /// What each screen's form holds, keyed by screen. Above the stack rather
  /// than inside a screen, because going somewhere and coming back must not
  /// lose what somebody had already typed — and because a screen that returns a
  /// value writes into the form of the one that opened it.
  final Map<String, Map<String, Object?>> _forms = {};

  Incoming get _in => widget.incoming;

  Map<String, dynamic>? get _current {
    final screens = _in.screens.cast<Map<String, dynamic>>();
    if (screens.isEmpty) return null;
    final byName = {for (final s in screens) s['name'] as String: s};
    for (final name in _stack.reversed) {
      final found = byName[name];
      if (found != null) return found;
    }
    return _opening;
  }

  /// Where the package opens: the screen that says so, or the only one there is.
  Map<String, dynamic> get _opening {
    final screens = _in.screens.cast<Map<String, dynamic>>();
    return screens.firstWhere((s) => s['start'] == true, orElse: () => screens.first);
  }

  void _navigate(String screen, [Map<String, Object?> args = const {}]) {
    setState(() {
      if (_stack.isEmpty) _stack.add(_opening['name'] as String);
      _stack.add(screen);
    });
    if (args.isNotEmpty) widget.onScreen?.call(screen, args);
  }

  void _back([Map<String, Object?> with_ = const {}]) => setState(() {
        if (_stack.length > 1) _stack.removeLast();
        if (with_.isEmpty) return;
        // What the screen returned, written into the form of the one that
        // opened it — the same names its fields write into.
        final below = _stack.isEmpty ? _opening['name'] as String : _stack.last;
        (_forms[below] ??= {}).addAll(with_);
      });

  @override
  Widget build(BuildContext context) {
    final screen = _current;
    if (screen == null) return const SizedBox.shrink();
    return _Cards(
      draw: widget.cards,
      choose: widget.chooser,
      child: _Screen(
      screen: screen,
      incoming: _in,
      canGoBack: _stack.length > 1,
      onNavigate: _navigate,
      onBack: _back,
      onTap: widget.onTap,
      form: _forms[screen['name'] as String] ??= {},
      ),
    );
  }
}

/// The host's own card renderer, reachable from wherever a node asks for one.
class _Cards extends InheritedWidget {
  const _Cards({required this.draw, required this.choose, required super.child});

  final Widget Function(BuildContext context, String credential)? draw;
  final Future<String?> Function(BuildContext, String, List<String>)? choose;

  static Widget Function(BuildContext, String)? of(BuildContext c) =>
      c.dependOnInheritedWidgetOfExactType<_Cards>()?.draw;

  static Future<String?> Function(BuildContext, String, List<String>)? chooser(BuildContext c) =>
      c.dependOnInheritedWidgetOfExactType<_Cards>()?.choose;

  @override
  bool updateShouldNotify(_Cards old) => draw != old.draw || choose != old.choose;
}

/// Formatting, which is the half of "the host does it" that belongs to a
/// toolkit. Resolving — which credentials, in what order, how many — happens in
/// the compiler, because that is where `limit`, `order by` and `verified with`
/// are defined and a second implementation of them is a second set of rules.
class Format {
  Format._();

  /// A number a person reads. The application never touches one: it would get
  /// the thousands separator, the era and the currency position wrong
  /// separately from every other application.
  static String value(Object? v, String locale) {
    if (v == null) return '—';
    if (v is bool) return v ? 'yes' : 'no';
    if (v is int) {
      // A time, or a number. Milliseconds since the epoch is the only thing in
      // this range, and a points balance is never within a century of it.
      if (v > 1500000000000) return _date(DateTime.fromMillisecondsSinceEpoch(v, isUtc: true), locale);
      return _thousands(v);
    }
    if (v is Map && v['credential'] != null) return v['credential'] as String;
    return v.toString();
  }

  static String _thousands(int n) {
    final digits = n.abs().toString();
    final out = StringBuffer(n < 0 ? '-' : '');
    for (var i = 0; i < digits.length; i++) {
      if (i > 0 && (digits.length - i) % 3 == 0) out.write(',');
      out.write(digits[i]);
    }
    return out.toString();
  }

  static const _months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
  static const _thai = ['ม.ค.', 'ก.พ.', 'มี.ค.', 'เม.ย.', 'พ.ค.', 'มิ.ย.', 'ก.ค.', 'ส.ค.', 'ก.ย.', 'ต.ค.', 'พ.ย.', 'ธ.ค.'];

  /// Buddhist era in Thai — the sort of thing that is wrong in forty
  /// applications the moment forty applications are allowed to do it.
  static String _date(DateTime at, String locale) => locale == 'th'
      ? '${at.day} ${_thai[at.month - 1]} ${at.year + 543}'
      : '${at.day} ${_months[at.month - 1]} ${at.year}';
}


/// The first host's tokens, copied from `vaulet-app/lib/core/theme/theme.dart`.
///
/// A copy, and the copy is the point: this preview shows what a VAL screen
/// looks like **on Vaulet**, and a second host would supply its own. The
/// language repository holds no theme; a renderer does, because a renderer is
/// something a host is.
///
/// Kept to tokens rather than the whole file. What drifts here is cosmetic and
/// visible; what would drift if the catalogue's *semantics* were copied is not.
class Vaulet {
  Vaulet._();

  /// Teal ink — the old brand teal taken almost to black. Near-black rather
  /// than black because `#000000` reads as a value nobody set.
  static const seed = Color(0xFF10201C);

  /// Success / verified green.
  static const verified = Color(0xFF2E7D32);

  // The 4-based spacing scale, so gaps stay on one rhythm.
  static const xs = 4.0;
  static const sm = 8.0;
  static const md = 12.0;
  static const lg = 16.0;
  static const xxl = 24.0;

  /// Buttons and cards share 14; sheets use 20.
  static const radiusCard = 14.0;

  /// A filled or outlined button is 52 tall, readable and easy to tap.
  static const buttonHeight = 52.0;

  static const cardTitle = TextStyle(fontSize: 16, fontWeight: FontWeight.w700);

  /// A section subhead: uppercase, small, letter-spaced, on `outline`. Padding
  /// `(16, 16, 16, 8)` — the wallet's, not Material's.
  static const sectionLabel = TextStyle(fontSize: 11, letterSpacing: 1);
  static const sectionPad = EdgeInsets.fromLTRB(lg, lg, lg, sm);

  /// The app-wide inset under a docked action: `(16, 8, 16, 24)`.
  static const bottomBarInset = EdgeInsets.fromLTRB(16, sm, 16, xxl);

  /// `surfaceContainerHighest @ 0.4` — the subtle grouped-card fill, and the
  /// reason a card here does not look like a Material card.
  static Color cardFill(ColorScheme s) =>
      s.surfaceContainerHighest.withValues(alpha: 0.4);

  /// A dark accent has to be set, not seeded: `fromSeed` rebuilds a seed's hue
  /// at a fixed lightness, so asking it for near-black hands back the mid-tone
  /// it started from. On a light ground the accent *is* the colour that was
  /// chosen; dark keeps Material's derivation, because near-black on a
  /// near-black ground is not an accent.
  /// The spacing scale, by the word a screen used. A screen names a step and
  /// never a number, so one phone's idea of `loose` is the same everywhere on it.
  static double gapOf(Object? word) => switch (word) {
        'tight' => sm,
        'loose' => xxl,
        _ => md,
      };

  static ThemeData theme(Brightness brightness) {
    var scheme = ColorScheme.fromSeed(seedColor: seed, brightness: brightness);
    if (brightness == Brightness.light) {
      scheme = scheme.copyWith(primary: seed, onPrimary: Colors.white);
    }
    final buttonShape = RoundedRectangleBorder(borderRadius: BorderRadius.circular(radiusCard));
    // A button declares its own text style, which replaces the theme's rather
    // than adding to it — so the Thai fallback has to be repeated here or every
    // button label goes back to being boxes. This is the same edge the wallet's
    // own theme documents.
    const buttonText = TextStyle(
      fontSize: 15,
      fontWeight: FontWeight.w600,
      fontFamilyFallback: ['Anuphan'],
    );
    return ThemeData(
      // Latin from the default font, Thai from the one bundled beside it. A
      // fallback rather than a family, so the preview keeps looking like the
      // wallet in English and stops drawing boxes in Thai.
      fontFamilyFallback: const ['Anuphan'],
      useMaterial3: true,
      colorScheme: scheme,
      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(
          minimumSize: const Size.fromHeight(buttonHeight),
          textStyle: buttonText,
          shape: buttonShape,
        ),
      ),
      outlinedButtonTheme: OutlinedButtonThemeData(
        style: OutlinedButton.styleFrom(
          minimumSize: const Size.fromHeight(buttonHeight),
          textStyle: buttonText,
          shape: buttonShape,
        ),
      ),
      cardTheme: CardThemeData(
        margin: EdgeInsets.zero,
        elevation: 0,
        color: cardFill(scheme),
        clipBehavior: Clip.antiAlias,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(radiusCard)),
      ),
      // One leading column for every row in the app, so a tile with a 28pt icon
      // and one with a 28pt swatch put their titles in the same place.
      listTileTheme: const ListTileThemeData(minLeadingWidth: 36),
      appBarTheme: AppBarTheme(centerTitle: false, backgroundColor: scheme.surface, elevation: 0),
      // Fainter and thinner than Material's default, app-wide.
      dividerTheme: DividerThemeData(
        color: scheme.outlineVariant.withValues(alpha: 0.4),
        thickness: 0.5,
      ),
    );
  }
}


/// What the playground posts in: the parsed screen, the signed text bundle, the
/// locale and the theme. Nothing here is computed from source — the compiler
/// did that, and a renderer that parsed VAL would be a second front end to keep
/// faithful to the first.
class Incoming {
  const Incoming({
    required this.screens,
    required this.text,
    required this.locale,
    required this.dark,
  });

  final List<dynamic> screens;
  final Map<String, dynamic> text;
  final String locale;
  final bool dark;

  static const empty = Incoming(screens: [], text: {}, locale: 'en', dark: false);

  /// Whether a name is one of this package's screens, which is what decides
  /// whether a press moves or runs an action.
  bool hasScreen(String name) =>
      screens.any((s) => (s as Map<String, dynamic>)['name'] == name);

  factory Incoming.fromJson(Map<String, dynamic> j) => Incoming(
    screens: (j['screens'] as List?) ?? const [],
    text: (j['text'] as Map?)?.cast<String, dynamic>() ?? const {},
    locale: (j['locale'] as String?) ?? 'en',
    dark: (j['dark'] as bool?) ?? false,
  );


}

/// Which components are answered rather than read. They are drawn as one group
/// where they sit together, the way the wallet's attribute rows are.
bool _isInput(Map<String, dynamic> n) =>
    const {'field', 'toggle', 'pick'}.contains(n['kind']);

bool _isButton(Map<String, dynamic> n) => n['kind'] == 'button';


/// Consecutive inputs, gathered. Runs of one non-input are left alone.
///
/// The application never declares a group: it lists the fields it needs and the
/// host puts them on one surface. A screen that had to say `group { … }` is a
/// screen that can forget to.
List<List<Map<String, dynamic>>> _group(List<Map<String, dynamic>> children) {
  final out = <List<Map<String, dynamic>>>[];
  for (final c in children) {
    if (_isInput(c) && out.isNotEmpty && _isInput(out.last.first)) {
      out.last.add(c);
    } else {
      out.add([c]);
    }
  }
  return out;
}


/// The icon a field carries, from the catalogue's closed set. An unknown word
/// draws the neutral one rather than nothing, because the check that refuses it
/// has already run by the time anything is drawn.
IconData _iconOf(Object? word) => switch (word) {
      'receipt' => Icons.receipt_long_outlined,
      'wallet' => Icons.account_balance_wallet_outlined,
      'shield' => Icons.shield_outlined,
      'person' => Icons.person_outline,
      'card' => Icons.credit_card,
      'calendar' => Icons.calendar_today_outlined,
      'location' => Icons.location_on_outlined,
      'money' => Icons.payments_outlined,
      'check' => Icons.check_circle_outline,
      'warning' => Icons.warning_amber_outlined,
      'document' => Icons.description_outlined,
      'key' => Icons.key_outlined,
      _ => Icons.edit_outlined,
    };


/// Pages side by side, with dots under them.
///
/// The page it is on is the host's — like a scroll position, it is not
/// application state, it is not hashed and it is not in the record.
class _Carousel extends StatefulWidget {
  const _Carousel({required this.pages, required this.incoming});

  final List<Map<String, dynamic>> pages;
  final Incoming incoming;

  @override
  State<_Carousel> createState() => _CarouselState();
}

class _CarouselState extends State<_Carousel> {
  /// Far enough from either end that a person swiping back on the first page
  /// finds the last one there, rather than a wall. The pages repeat; the count
  /// only has to outlast a session.
  static const _middle = 10000;

  late final _controller = PageController(initialPage: _middle);
  int _page = 0;

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final n = widget.pages.length;
    return Column(
      children: [
        // Out past the screen's own padding: a banner that stops short of the
        // edge reads as a card that failed to fill its row, and the wallet's
        // own home screen runs them to the edge.
        // The height is bounded here rather than inside: an OverflowBox in a
        // column is asked for an unbounded height and throws, which took the
        // whole screen with it.
        SizedBox(
          height: 140,
          child: OverflowBox(
            // The width of the screen this is drawn on, which the host says.
            // A carousel bleeds to the edges past the page's padding, and the
            // number to bleed to is the device's rather than one written here.
            maxWidth: MediaQuery.sizeOf(context).width,
            child: SizedBox(
              width: MediaQuery.sizeOf(context).width,
              child: PageView.builder(
                controller: _controller,
                onPageChanged: (i) => setState(() => _page = i % n),
                itemBuilder: (context, i) =>
                    _Node(node: widget.pages[i % n], incoming: widget.incoming),
              ),
            ),
          ),
        ),
        if (widget.pages.length > 1)
          Padding(
            // Room on both sides. Dots against the card read as part of it.
            padding: const EdgeInsets.only(top: Vaulet.lg, bottom: Vaulet.sm),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                for (var i = 0; i < widget.pages.length; i++)
                  Container(
                    width: i == _page ? 18 : 6,
                    height: 6,
                    margin: const EdgeInsets.symmetric(horizontal: 3),
                    decoration: BoxDecoration(
                      color: i == _page
                          ? scheme.primary
                          : scheme.onSurfaceVariant.withValues(alpha: 0.3),
                      borderRadius: BorderRadius.circular(999),
                    ),
                  ),
              ],
            ),
          ),
      ],
    );
  }
}


/// The white group an answered row sits in: one card, hairlines between rows.
class _InputGroup extends StatelessWidget {
  const _InputGroup({required this.rows, required this.incoming});

  final List<Map<String, dynamic>> rows;
  final Incoming incoming;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Card(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          for (var i = 0; i < rows.length; i++) ...[
            Padding(
              padding: const EdgeInsets.fromLTRB(Vaulet.lg, Vaulet.md, Vaulet.md, Vaulet.md),
              child: _Node(node: rows[i], incoming: incoming),
            ),
            if (i != rows.length - 1)
              Divider(
                height: 1,
                thickness: 0.5,
                indent: Vaulet.lg,
                color: scheme.outlineVariant.withValues(alpha: 0.4),
              ),
          ],
        ],
      ),
    );
  }
}


/// Where a press can go, for the nodes deep in a screen.
///
/// Moving between screens is the host's: it never reaches the application, so
/// nothing about it is posted out and nothing about it is in the record.
class _Nav extends InheritedWidget {
  const _Nav({
    required this.go,
    required this.back,
    required this.run,
    required super.child,
  });

  /// A press on something that names an action. What leaves the renderer: the
  /// host runs it, and everything else about a press — moving, coming back —
  /// never reaches the application at all.
  final void Function(String, Map<String, Object?>) run;

  /// True when the target was a screen and the move happened.
  final bool Function(String, Map<String, Object?>) go;

  /// Coming back, with whatever the screen returned.
  final void Function(Map<String, Object?>) back;

  static _Nav? of(BuildContext c) => c.dependOnInheritedWidgetOfExactType<_Nav>();

  @override
  bool updateShouldNotify(_Nav old) => false;
}


/// What the form holds, until it is submitted.
///
/// The host's, not the application's: a scroll position or a half-typed field in
/// application state would be hashed, signed and replayed, and "provable" would
/// mean less by one press each time.
/// A form control and the words beside it.
///
/// Not a `ListTile`: those carry an `InkWell`, so the whole row lit up when it
/// was touched — a ripple across a line whose only pressable thing is the
/// control at the end of it. The label is still a tap target, because a person
/// aiming at "Remind me" means the switch; it just does not announce itself as
/// a surface.
Widget _control(
  BuildContext context, {
  Widget? leading,
  required Widget label,
  Widget? control,
  VoidCallback? onTap,
}) {
  return GestureDetector(
    onTap: onTap,
    behavior: HitTestBehavior.opaque,
    child: Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Row(
        children: [
          if (leading != null) ...[leading, const SizedBox(width: 4)],
          Expanded(child: label),
          if (control != null) control,
        ],
      ),
    ),
  );
}

class _Form extends InheritedWidget {
  const _Form({required this.values, required this.onChanged, required super.child});

  final Map<String, Object?> values;
  final void Function(String, Object?) onChanged;

  static Map<String, Object?> of(BuildContext c) =>
      c.dependOnInheritedWidgetOfExactType<_Form>()?.values ?? const {};

  static void set(BuildContext c, String name, Object? value) =>
      c.getInheritedWidgetOfExactType<_Form>()?.onChanged(name, value);

  @override
  bool updateShouldNotify(_Form old) => old.values != values;
}


/// A screen, built the way the wallet builds one: an `AppBar`, a scrolling body
/// on the standard 16pt gutter, and the primary action **docked at the bottom**
/// rather than sitting inline in the content.
///
/// The last one is a layout fact rather than a preference. A screen whose call
/// to action scrolls with the content is a screen where it is sometimes not on
/// screen, and the wallet answered that once — in `BottomActionBar`, for every
/// screen — so a preview that put the button inline would be showing a layout
/// the host does not produce.
class _Screen extends StatefulWidget {
  const _Screen({
    required this.screen,
    required this.incoming,
    required this.onTap,
    this.canGoBack = false,
    this.onNavigate,
    this.onBack,
    this.form,
  });

  final Map<String, dynamic> screen;
  final Incoming incoming;
  final void Function(String, Map<String, Object?>) onTap;
  final bool canGoBack;
  final void Function(String, [Map<String, Object?>])? onNavigate;
  final void Function([Map<String, Object?>])? onBack;
  final Map<String, Object?>? form;

  @override
  State<_Screen> createState() => _ScreenState();
}

class _ScreenState extends State<_Screen> {
  Map<String, Object?> get _values => widget.form ?? _own;
  final Map<String, Object?> _own = {};

  static bool _isPrimary(Map<String, dynamic> n) =>
      n['kind'] == 'button' && ((n['args'] as Map?)?['emphasis'] as String?)?.trim() == 'primary';

  static List<Map<String, dynamic>> _flatten(List<Map<String, dynamic>> ns) => [
        for (final n in ns) ...[
          n,
          ..._flatten(((n['children'] as List?) ?? const []).cast<Map<String, dynamic>>()),
        ],
      ];

  /// The docked action is taken *out* of the tree, not filtered off the top of
  /// it. Filtering the top level left a button nested in a `column` in both
  /// places at once — drawn inline and docked, which is a layout no host
  /// produces.
  static List<Map<String, dynamic>> _prune(List<Map<String, dynamic>> ns) => [
        for (final n in ns)
          if (!_isPrimary(n))
            {
              ...n,
              'children': _prune(((n['children'] as List?) ?? const []).cast<Map<String, dynamic>>()),
            },
      ];

  @override
  Widget build(BuildContext context) {
    final nodes = ((widget.screen['tree'] as List?) ?? const []).cast<Map<String, dynamic>>();
    final docked = _flatten(nodes).where(_isPrimary).toList();

    final failed = widget.screen['error'] as String?;
    if (failed != null) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Text(
            'This screen did not resolve.\n$failed',
            textAlign: TextAlign.center,
            style: TextStyle(color: Theme.of(context).colorScheme.error, fontSize: 13),
          ),
        ),
      );
    }

    return _Nav(
      run: widget.onTap,
      back: (returned) => widget.onBack?.call(returned),
      go: (target, args) {
        final onNavigate = widget.onNavigate;
        if (onNavigate == null || !widget.incoming.hasScreen(target)) return false;
        onNavigate(target, args);
        return true;
      },
      child: _Form(
        values: _values,
        onChanged: (name, value) => setState(() => _values[name] = value),
        child: Scaffold(
      backgroundColor: Theme.of(context).colorScheme.surface,
      appBar: AppBar(
        toolbarHeight: 52,
        // Host chrome. An application declares where a press goes and never how
        // to come back, so this is drawn here whenever there is somewhere to
        // come back to.
        leading: widget.canGoBack
            ? IconButton(
                icon: const Icon(Icons.arrow_back),
                onPressed: widget.onBack,
              )
            : null,
        automaticallyImplyLeading: false,
        // The screen's own sentence where it has one. The identifier behind it
        // is ASCII, so a title taken from it could never be Thai — which is
        // what it was doing until screens could carry a title.
        title: widget.screen['title'] == null
            ? Text(
                widget.screen['name'] as String? ?? '',
                style: const TextStyle(fontSize: 20, fontWeight: FontWeight.w700),
              )
            : _Node(
                node: widget.screen['title'] as Map<String, dynamic>,
                incoming: widget.incoming,
              ),
      ),
      // **The screen is a stack too.** A node at the top level saying
      // `position: absolute` is placed against the screen — a floating button,
      // a banner over the content, a watermark — rather than needing a `stack`
      // wrapped around everything to say so. Where a thing sits is read by
      // whoever holds the flow, and at the top level that is the screen.
      body: Stack(
        children: [
          ListView(
            // **The gutter is per node, not on the list.** A chat list runs to
            // both edges and everything else is held in from them; with the
            // padding on the list there was no way for one child to decline
            // it, and taking the width back with a negative margin is what
            // broke layout. A node that sets its own `margin` has said where
            // it sits, so the screen does not put it anywhere.
            padding: const EdgeInsets.symmetric(vertical: Vaulet.sm),
            children: [
              for (final n in _prune(nodes))
                if (!_floats(n))
                  Padding(
                    padding: _argsOf(n).containsKey('margin')
                        ? EdgeInsets.zero
                        : const EdgeInsets.symmetric(horizontal: kScreenPadH),
                    child: _Node(node: n, incoming: widget.incoming),
                  ),
            ],
          ),
          for (final n in _prune(nodes))
            if (_floats(n))
              Positioned(
                top: _sizeOf(_argsOf(n)['top']),
                right: _sizeOf(_argsOf(n)['right']),
                bottom: _sizeOf(_argsOf(n)['bottom']),
                left: _sizeOf(_argsOf(n)['left']),
                child: _Node(node: n, incoming: widget.incoming),
              ),
        ],
      ),
      bottomNavigationBar: docked.isEmpty
          ? null
          : Padding(
              padding: Vaulet.bottomBarInset,
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  for (final b in docked)
                    SizedBox(width: double.infinity, child: _Node(node: b, incoming: widget.incoming)),
                ],
              ),
            ),
        ),
      ),
    );
  }
}


Map<String, dynamic> _argsOf(Map<String, dynamic> n) =>
    ((n['args'] as Map?) ?? const {}).cast<String, dynamic>();

/// Whether a node asked to be placed rather than laid out in the flow.
bool _floats(Map<String, dynamic> n) => _argsOf(n)['position'] == 'absolute';

/// The wallet's standard horizontal gutter.
const double kScreenPadH = 16;


/// A size the interface asked for: a word, or a number of its own.
double? _sizeOf(Object? v) => switch (v) {
      final int n => n.toDouble(),
      'fill' => double.infinity,
      _ => null,
    };


/// Space from the scale, or a number of its own. The scale is what makes an
/// application look like it belongs here; a number is the application saying
/// what it wants, which is allowed.
double _spaceOf(Object? v, {double fallback = Vaulet.md}) => switch (v) {
      final int n => n.toDouble(),
      'none' => 0,
      'tight' => Vaulet.sm,
      'normal' => Vaulet.md,
      'loose' => Vaulet.xxl,
      _ => fallback,
    };

/// An inset: one value for every side, two for vertical and horizontal, or
/// four written the way CSS writes them — top, right, bottom, left.
///
/// **A token first, a list when a token will not do.** A number or a word is
/// the whole of what most screens need; a list is the publisher saying this
/// side and not that one, which no token can express.
EdgeInsets _edgeOf(Object? v) {
  if (v is List) {
    final n = [for (final e in v) _spaceOf(e, fallback: 0)];
    return switch (n.length) {
      0 => EdgeInsets.zero,
      1 => EdgeInsets.all(n[0]),
      2 => EdgeInsets.symmetric(vertical: n[0], horizontal: n[1]),
      3 => EdgeInsets.only(top: n[0], right: n[1], bottom: n[2], left: n[1]),
      _ => EdgeInsets.only(top: n[0], right: n[1], bottom: n[2], left: n[3]),
    };
  }
  return EdgeInsets.all(_spaceOf(v, fallback: 0));
}

/// A corner radius: one for all four, or four of their own.
BorderRadius? _radiusOf(Object? v) {
  if (v is List) {
    final n = [for (final e in v) _sizeOf(e) ?? 0];
    if (n.isEmpty) return null;
    final four = [for (var i = 0; i < 4; i++) n[i % n.length]];
    return BorderRadius.only(
      topLeft: Radius.circular(four[0]),
      topRight: Radius.circular(four[1]),
      bottomRight: Radius.circular(four[2]),
      bottomLeft: Radius.circular(four[3]),
    );
  }
  final one = _sizeOf(v);
  return one == null ? null : BorderRadius.circular(one);
}

/// A colour the interface asked for: a token from the scale, or a value of its
/// own.
///
/// **Both, on purpose.** The tokens are what make an application look like it
/// belongs on this phone and follow the person's light or dark setting; a hex
/// value is the publisher saying what their product looks like, which the
/// registry's own note calls guidance rather than a fence. What no application
/// gets is the wallet's chrome — that is drawn outside anything a package
/// reaches.
Color? _colorOf(Object? v, ColorScheme scheme) {
  if (v is! String || v.isEmpty) return null;
  if (v.startsWith('#')) {
    final hex = v.substring(1);
    final value = int.tryParse(hex.length == 6 ? 'ff$hex' : hex, radix: 16);
    return value == null ? null : Color(value);
  }
  return switch (v) {
    'foreground.primary' => scheme.onSurface,
    'foreground.secondary' => scheme.onSurfaceVariant,
    'foreground.muted' => scheme.outline,
    'background.primary' => scheme.surface,
    'background.raised' => scheme.surfaceContainerHighest,
    'accent' => scheme.primary,
    'danger' => scheme.error,
    'warning' => scheme.tertiary,
    'success' => scheme.secondary,
    _ => null,
  };
}

FontWeight? _weightOf(Object? v) => switch (v) {
      'regular' => FontWeight.w400,
      'medium' => FontWeight.w600,
      'bold' => FontWeight.w700,
      final int n => FontWeight.values[(n ~/ 100 - 1).clamp(0, 8)],
      _ => null,
    };

TextAlign? _textAlignOf(Object? v) => switch (v) {
      'start' => TextAlign.start,
      'center' => TextAlign.center,
      'end' => TextAlign.end,
      _ => null,
    };

CrossAxisAlignment _crossOf(Object? v, CrossAxisAlignment fallback) => switch (v) {
      'start' => CrossAxisAlignment.start,
      'center' => CrossAxisAlignment.center,
      'end' => CrossAxisAlignment.end,
      'stretch' => CrossAxisAlignment.stretch,
      _ => fallback,
    };

MainAxisAlignment _mainOf(Object? v, MainAxisAlignment fallback) => switch (v) {
      'start' => MainAxisAlignment.start,
      'center' => MainAxisAlignment.center,
      'end' => MainAxisAlignment.end,
      'between' => MainAxisAlignment.spaceBetween,
      _ => fallback,
    };

List<BoxShadow> _shadowOf(Object? v, ColorScheme scheme) {
  // Written out: a colour, a blur, an offset, a spread. The tokens above are
  // two shadows somebody chose; a product with its own is not wrong for having
  // one.
  if (v is Map) {
    return [
      BoxShadow(
        color: _colorOf(v['color'], scheme) ?? scheme.shadow.withValues(alpha: 0.2),
        blurRadius: _sizeOf(v['blur']) ?? 12,
        spreadRadius: _sizeOf(v['spread']) ?? 0,
        offset: Offset(_sizeOf(v['x']) ?? 0, _sizeOf(v['y']) ?? 4),
      ),
    ];
  }
  return switch (v) {
    'raised' => [BoxShadow(color: scheme.shadow.withValues(alpha: 0.10), blurRadius: 8, offset: const Offset(0, 2))],
    'floating' => [BoxShadow(color: scheme.shadow.withValues(alpha: 0.18), blurRadius: 24, offset: const Offset(0, 8))],
    _ => const [],
  };
}

/// A gradient: two colours and a diagonal, or one written out with its own
/// angle and stops.
Gradient? _gradientOf(Object? v, ColorScheme scheme) {
  List<Color> colours(List raw) => [
        for (final c in raw) _colorOf(c, scheme) ?? scheme.primary,
      ];
  if (v is List && v.length >= 2) {
    return LinearGradient(
      begin: Alignment.topLeft,
      end: Alignment.bottomRight,
      colors: colours(v),
    );
  }
  if (v is Map && v['colors'] is List && (v['colors'] as List).length >= 2) {
    final stops = v['stops'];
    // Degrees, clockwise from the top, because that is how a designer says it.
    final angle = ((_sizeOf(v['angle']) ?? 135) - 90) * math.pi / 180;
    return v['shape'] == 'radial'
        ? RadialGradient(
            colors: colours(v['colors'] as List),
            stops: stops is List ? [for (final s in stops) (_sizeOf(s) ?? 0) / 100] : null,
            radius: (_sizeOf(v['radius']) ?? 100) / 100,
          )
        : LinearGradient(
            begin: Alignment(-math.cos(angle), -math.sin(angle)),
            end: Alignment(math.cos(angle), math.sin(angle)),
            colors: colours(v['colors'] as List),
            stops: stops is List ? [for (final s in stops) (_sizeOf(s) ?? 0) / 100] : null,
          );
  }
  return null;
}

/// Turned, scaled or moved. Degrees and hundredths, because a screen
/// description has no decimals.
Matrix4? _transformOf(Object? v) {
  if (v is! Map) return null;
  final m = Matrix4.identity();
  if (_sizeOf(v['translateX']) != null || _sizeOf(v['translateY']) != null) {
    m.translateByDouble(_sizeOf(v['translateX']) ?? 0, _sizeOf(v['translateY']) ?? 0, 0, 1);
  }
  if (_sizeOf(v['rotate']) case final double d) {
    m.rotateZ(d * math.pi / 180);
  }
  if (_sizeOf(v['scale']) case final double sc) {
    m.scaleByDouble(sc / 100, sc / 100, 1, 1);
  }
  return m;
}

/// The type scale, for `style: title` and the rest of the tokens.
TextStyle? _typeOf(Object? v) => switch (v) {
      'title' => const TextStyle(fontSize: 22, fontWeight: FontWeight.w700),
      'heading' => const TextStyle(fontSize: 17, fontWeight: FontWeight.w600),
      'body' => const TextStyle(fontSize: 15),
      'caption' => const TextStyle(fontSize: 12),
      'mono' => const TextStyle(fontFamily: 'Menlo', fontSize: 13),
      _ => null,
    };

/// Marks the rows a card holds as a group, so a row draws flush instead of
/// wearing a card of its own.
class _Grouped extends InheritedWidget {
  const _Grouped({required super.child});

  static bool of(BuildContext c) =>
      c.dependOnInheritedWidgetOfExactType<_Grouped>() != null;

  @override
  bool updateShouldNotify(_Grouped old) => false;
}

class _Node extends StatefulWidget {
  const _Node({required this.node, required this.incoming});

  final Map<String, dynamic> node;
  final Incoming incoming;

  @override
  State<_Node> createState() => _NodeState();
}

class _NodeState extends State<_Node> {
  Map<String, dynamic> get n => widget.node;
  List<Map<String, dynamic>> get children =>
      ((n['children'] as List?) ?? const []).cast<Map<String, dynamic>>();

  Map<String, dynamic> get args => ((n['args'] as Map?) ?? const {}).cast<String, dynamic>();

  /// Whether this row is one of a group under a single rounded edge.
  bool _grouped(BuildContext context) => _Grouped.of(context);

  /// The template is signed and the slots are not. The host fills and formats;
  /// an application that formatted a number would get Thai digits, the
  /// thousands separator and the currency position wrong separately from every
  /// other application.
  /// The same sentence as [_text], as a plain string, for the places a toolkit
  /// wants one — a field's label cannot be a widget.
  String _label() {
    final key = args['text'] as String?;
    if (key == null) return '';
    final entry = (widget.incoming.text[key] as Map?)?.cast<String, dynamic>();
    return entry?[widget.incoming.locale] as String? ?? key;
  }

  /// The supporting line, resolved the same way the title is: words written in
  /// place, or a key where the package has a bundle.
  String _detail() {
    final key = args['detail'] as String?;
    if (key == null) return '';
    final entry = (widget.incoming.text[key] as Map?)?.cast<String, dynamic>();
    return entry?[widget.incoming.locale] as String? ?? key;
  }

  /// A value as a person reads it: a sentence from the bundle when the package
  /// wrote a key, and formatted by the host when it is a number, a date or a
  /// word of its own.
  String _say(Object? v) {
    if (v is String) {
      final entry = (widget.incoming.text[v] as Map?)?.cast<String, dynamic>();
      final said = entry?[widget.incoming.locale] as String? ?? entry?['en'] as String?;
      if (said != null) return said;
    }
    return v == null ? '' : Format.value(v, widget.incoming.locale);
  }

  Widget _text({TextStyle? style, bool upper = false}) {
    final key = args['text'] as String?;
    if (key == null) return const SizedBox.shrink();
    // Words written in place are the words. A package in one language has no
    // bundle at all, and 80% of them never will — a key is something to learn
    // about on the day a second language is added, and not before.
    final entry = (widget.incoming.text[key] as Map?)?.cast<String, dynamic>();
    var template = entry == null ? key : entry[widget.incoming.locale] as String?;
    if (upper) template = template?.toUpperCase();
    if (template == null) {
      return Text(
        '“$key” has no ${widget.incoming.locale}',
        style: TextStyle(color: Theme.of(context).colorScheme.error, fontSize: 12),
      );
    }

    final spans = <InlineSpan>[];
    // A slot is named like an identifier, digits included: `{item2}` is a
    // name somebody writes, and a template's fallback slots are `{v0}`, `{v1}`.
    final pattern = RegExp(r'\{([a-zA-Z_][a-zA-Z0-9_]*)\}');
    var at = 0;
    for (final m in pattern.allMatches(template)) {
      if (m.start > at) spans.add(TextSpan(text: template.substring(at, m.start)));
      // A value, resolved by the compiler against the wallet. This side
      // formats it and does not look anything up.
      final value = args[m.group(1)];
      spans.add(
        TextSpan(
          text: value == null ? '${m.group(1)}?' : Format.value(value, widget.incoming.locale),
          style: TextStyle(
            fontWeight: FontWeight.w600,
            color: value == null ? Theme.of(context).colorScheme.error : null,
          ),
        ),
      );
      at = m.end;
    }
    if (at < template.length) spans.add(TextSpan(text: template.substring(at)));

    // **The node's own type props, over whatever this case chose.** Declared in
    // `common.style` and ignored until now, so a package asking for a size, a
    // weight or a colour got the catalogue's defaults and no word about it.
    // The case's style is the floor rather than the ceiling: a package that
    // says nothing still looks like it belongs here.
    final scheme = Theme.of(context).colorScheme;
    var out = style ?? const TextStyle(fontSize: 13);
    if (_typeOf(args['style']) case final TextStyle t) out = out.merge(t);
    out = out.copyWith(
      color: _colorOf(args['color'], scheme) ?? out.color,
      fontSize: _sizeOf(args['fontSize']) ?? out.fontSize,
      fontWeight: _weightOf(args['fontWeight']) ?? out.fontWeight,
      fontFamily: args['fontFamily'] as String? ?? out.fontFamily,
      fontStyle: args['italic'] == true ? FontStyle.italic : out.fontStyle,
      decoration: args['underline'] == true ? TextDecoration.underline : out.decoration,
      height: _sizeOf(args['lineHeight']) == null
          ? out.height
          : _sizeOf(args['lineHeight'])! / 100,
      // **Hundredths, like the line height above it.** Read as points, `80`
      // put a space of eighty between every letter and spread four characters
      // across a card.
      letterSpacing: _sizeOf(args['letterSpacing']) == null
          ? out.letterSpacing
          : _sizeOf(args['letterSpacing'])! / 100,
    );
    return Text.rich(
      TextSpan(children: spans),
      style: out,
      textAlign: _textAlignOf(args['textAlign']),
      maxLines: args['maxLines'] as int?,
      overflow: args['ellipsis'] == true || args['maxLines'] != null
          ? TextOverflow.ellipsis
          : null,
    );
  }

  /// Nodes that already wire their own press, so wrapping them again would
  /// give a card two ripples and a button two runs of one action.
  static const _tapsItself = {'button', 'tile', 'card', 'link', 'banner', 'checkbox', 'toggle', 'pick', 'select', 'field', 'slider', 'radioGroup', 'datePicker', 'filePicker'};

  bool get _kindHandlesItsOwnTap => _tapsItself.contains(n['kind'] as String? ?? '');

  void _run(String action) {
    final nav = _Nav.of(context);
    if (nav == null) return;
    if (nav.go(action, const {})) return;
    nav.run(action, _Form.of(context));
  }

  @override
  Widget build(BuildContext context) {
    // Below a width, or above one: a screen that reads differently on a phone
    // and on a tablet says so per node rather than being drawn twice.
    final width = MediaQuery.sizeOf(context).width;
    final from = _sizeOf(args['visibleFrom']);
    final until = _sizeOf(args['visibleUntil']);
    if (from != null && width < from) return const SizedBox.shrink();
    if (until != null && width >= until) return const SizedBox.shrink();

    final drawn = _common(_draw(context));
    // A change of style, animated rather than jumped, when the package asks
    // for it in milliseconds. The default is no animation: a screen that moves
    // when nobody asked it to is a screen fighting the person reading it.
    if (args['animate'] case final int ms when ms > 0) {
      return AnimatedSize(
        duration: Duration(milliseconds: ms),
        curve: Curves.easeOutCubic,
        child: AnimatedSwitcher(duration: Duration(milliseconds: ms), child: drawn),
      );
    }
    return drawn;
  }

  /// Layout, accessibility and style, applied to whatever the node drew.
  ///
  /// Held here rather than inside each case, so a capability added later takes
  /// them without being asked and a renderer for another platform has one place
  /// to look.
  Widget _common(Widget child) {
    var out = child;
    if (args['visible'] == false) return const SizedBox.shrink();
    final scheme = Theme.of(context).colorScheme;

    // **The whole of `common`, not the third of it that was implemented.**
    // `hosts/core.json` has carried layout, accessibility and style on every
    // drawing node since the catalogue was written; the renderer read `flex`,
    // `width`, `height`, `padding` and `opacity` and dropped the rest on the
    // floor — a package could ask for a background, a radius, a weight or an
    // aspect ratio, be told it was provided, and see none of it.

    if (args['padding'] != null) {
      out = Padding(padding: _edgeOf(args['padding']), child: out);
    }

    // The painted box: background, border, radius and shadow together, because
    // a radius without the thing it clips is a prop that does nothing.
    final background = _colorOf(args['background'], scheme);
    final border = _colorOf(args['border'], scheme);
    final radius = _radiusOf(args['borderRadius']);
    final borderWidth = _sizeOf(args['borderWidth']) ?? 1;
    final shadow = _shadowOf(args['shadow'], scheme);
    final gradient = _gradientOf(args['gradient'], scheme);
    if (background != null ||
        border != null ||
        radius != null ||
        shadow.isNotEmpty ||
        gradient != null) {
      out = Container(
        clipBehavior: radius == null || args['clip'] == false ? Clip.none : Clip.antiAlias,
        decoration: BoxDecoration(
          color: gradient != null ? null : background,
          gradient: gradient,
          border: border == null ? null : Border.all(color: border, width: borderWidth),
          borderRadius: radius,
          boxShadow: shadow,
          shape: args['shape'] == 'circle' ? BoxShape.circle : BoxShape.rectangle,
        ),
        child: out,
      );
    }

    if (args['aspectRatio'] case final int ratio when ratio > 0) {
      // Written as hundredths, because a screen description has no decimals and
      // an ID-1 card is 1.586 rather than 2.
      out = AspectRatio(aspectRatio: ratio / 1000, child: out);
    }

    final w = _sizeOf(args['width']);
    final h = _sizeOf(args['height']);
    if (w != null || h != null) {
      out = SizedBox(width: w, height: h, child: out);
    }
    if (_sizeOf(args['minWidth']) != null ||
        _sizeOf(args['maxWidth']) != null ||
        _sizeOf(args['minHeight']) != null ||
        _sizeOf(args['maxHeight']) != null) {
      out = ConstrainedBox(
        constraints: BoxConstraints(
          minWidth: _sizeOf(args['minWidth']) ?? 0,
          maxWidth: _sizeOf(args['maxWidth']) ?? double.infinity,
          minHeight: _sizeOf(args['minHeight']) ?? 0,
          maxHeight: _sizeOf(args['maxHeight']) ?? double.infinity,
        ),
        child: out,
      );
    }
    if (args['margin'] != null) {
      // **Never negative.** A margin that took width back by measuring what
      // it was given and asking for more of it recursed through layout until
      // the stack ran out — and a screen that will not lay out is a screen
      // that draws nothing at all. Reaching the edge is the screen's gutter
      // not being applied, which is below.
      final inset = _edgeOf(args['margin']);
      if (inset.isNonNegative) out = Padding(padding: inset, child: out);
    }
    if (_transformOf(args['transform']) case final Matrix4 m) {
      out = Transform(transform: m, alignment: Alignment.center, child: out);
    }
    if (args['opacity'] case final int o) out = Opacity(opacity: o / 100, child: out);

    // Frosted: what is behind this node, blurred. Nothing leaves the phone and
    // nothing is fetched — it is the pixels already there.
    if (_sizeOf(args['blur']) case final double sigma when sigma > 0) {
      out = ClipRRect(
        borderRadius: _radiusOf(args['borderRadius']) ?? BorderRadius.zero,
        child: BackdropFilter(
          filter: ui.ImageFilter.blur(sigmaX: sigma, sigmaY: sigma),
          child: out,
        ),
      );
    }

    // **A press, on anything.** `onTap` was a prop of the handful of nodes that
    // happened to take one; a node a publisher designed themselves could not be
    // pressed at all. The ripple is the platform's, so a custom component still
    // feels like this phone.
    final tap = args['onTap'];
    final longPress = args['onLongPress'];
    final doubleTap = args['onDoubleTap'];
    if ((tap is String || longPress is String || doubleTap is String) &&
        !_kindHandlesItsOwnTap) {
      // **No ripple.** A splash spreading under a card with its own corners
      // paints a grey rectangle through them, and a press somewhere in a
      // designed screen is not always a button.
      out = GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: tap is String ? () => _run(tap) : null,
        onLongPress: longPress is String ? () => _run(longPress) : null,
        onDoubleTap: doubleTap is String ? () => _run(doubleTap) : null,
        child: out,
      );
    }

    if (args['flex'] case final int flex) out = Expanded(flex: flex, child: out);

    // What a screen reader is told. The words are the application's; the
    // mapping to VoiceOver or TalkBack is the platform renderer's, and never
    // something a Micro App reaches.
    final label = args['label'];
    if (label != null || args['role'] != null) {
      out = Semantics(
        label: label is String ? label : null,
        button: args['role'] == 'button',
        header: args['role'] == 'heading',
        image: args['role'] == 'image',
        enabled: args['disabled'] != true,
        selected: args['selected'] == true,
        child: out,
      );
    }
    return out;
  }

  Widget _draw(BuildContext context) {
    switch (n['kind'] as String? ?? '') {
      // Things drawn on top of each other. A child that says `position:
      // absolute` is placed by its own `top`/`right`/`bottom`/`left`; anything
      // else is laid in the middle, which is what a background and a label on
      // top of it are.
      case 'stack': {
        final clipped = args['clip'] == true;
        final stack = Stack(
          alignment: switch (args['align']) {
            'start' => Alignment.topLeft,
            'end' => Alignment.bottomRight,
            _ => Alignment.center,
          },
          // **Nothing is clipped unless somebody asks.** Half of what a stack
          // is for hangs off the edge — a badge, a corner mark — and clipping
          // by default cut every one of them. Asking is `clip: true`, and it
          // clips to this node's own corners: a square cut around a rounded
          // card leaves the decoration poking out of the corners, which is the
          // thing it was asked to stop.
          clipBehavior: Clip.none,
          children: [
            // **The stack places its own children and nobody else's.** A node
            // saying `position: absolute` three levels down, inside a column,
            // is not out of this stack's flow — it is in the column's — so the
            // placing is done here where the parent is known rather than by a
            // child asking what it happens to be inside.
            for (final c in children)
              () {
                final a = ((c['args'] as Map?) ?? const {}).cast<String, dynamic>();
                final node = _Node(node: c, incoming: widget.incoming);
                if (a['position'] != 'absolute') return node;
                return Positioned(
                  top: _sizeOf(a['top']),
                  right: _sizeOf(a['right']),
                  bottom: _sizeOf(a['bottom']),
                  left: _sizeOf(a['left']),
                  child: node,
                );
              }(),
          ],
        );
        if (!clipped) return stack;
        return ClipRRect(
          borderRadius: _radiusOf(args['borderRadius']) ?? BorderRadius.zero,
          child: stack,
        );
      }

      case 'column':
        final runs = _group(children);
        // A column that scrolls on its own, when the package says so. Declared
        // as `overflow` since the first registry and never read, so a screen
        // with a long list inside a fixed height simply overflowed.
        if (args['overflow'] == 'scroll') {
          return SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: _crossOf(args['align'], CrossAxisAlignment.stretch),
              children: [
                for (final c in children) _Node(node: c, incoming: widget.incoming),
              ],
            ),
          );
        }
        return Column(
          // **Not `min` when the column says where things go.** Shrink-wrapped,
          // `justify: between` has no space to distribute, so a card's header
          // and footer sat together in the middle of it.
          mainAxisSize:
              args['justify'] == null ? MainAxisSize.min : MainAxisSize.max,
          mainAxisAlignment: _mainOf(args['justify'], MainAxisAlignment.start),
          crossAxisAlignment: _crossOf(args['align'], CrossAxisAlignment.stretch),
          children: [
            for (var i = 0; i < runs.length; i++)
              Padding(
                // Buttons stacked on each other get more room than cards do:
                // they are targets somebody aims at, and two 52pt ones eight
                // points apart invite the mis-tap.
                padding: EdgeInsets.only(
                  bottom: args['gap'] != null
                      ? _spaceOf(args['gap'])
                      : _isButton(runs[i].first) &&
                              i + 1 < runs.length &&
                              _isButton(runs[i + 1].first)
                          ? Vaulet.md
                          : Vaulet.sm,
                ),
                child: runs[i].length == 1 && !_isInput(runs[i].first)
                    ? _Node(node: runs[i].first, incoming: widget.incoming)
                    : _InputGroup(rows: runs[i], incoming: widget.incoming),
              ),
          ],
        );

      // A section subhead, which is how the wallet groups a list — uppercase,
      // small, on `outline`, and never a heavier treatment invented per screen.
      case 'section':
        return Padding(
          padding: Vaulet.sectionPad,
          child: _text(
            style: Vaulet.sectionLabel.copyWith(color: Theme.of(context).colorScheme.outline),
            upper: true,
          ),
        );

      // A layout row. `tile` below is the one that carries a sentence — two
      // different things that shared a name until the catalogue was written
      // down and the collision had somewhere to show up.
      case 'row':
        return Padding(
          padding: EdgeInsets.symmetric(vertical: Vaulet.gapOf(args['gap'])),
          child: Row(
            mainAxisAlignment: _mainOf(args['justify'], MainAxisAlignment.start),
            crossAxisAlignment: _crossOf(args['align'], CrossAxisAlignment.center),
            children: [
              // Not wrapped in `Expanded` here: a child says `flex: 1` for
              // itself and the common props do it once. Doing both put an
              // Expanded inside an Expanded, which throws and takes the whole
              // preview with it.
              for (var i = 0; i < children.length; i++) ...[
                if (i > 0) SizedBox(width: _spaceOf(args['gap'], fallback: Vaulet.sm)),
                _Node(node: children[i], incoming: widget.incoming),
              ],
            ],
          ),
        );

      // `AppCard` + `ListTile`: the leading icon, the title, a subtitle capped
      // at two lines, and a chevron when a press goes somewhere. A bare row of
      // text would put the title in a different place from every other row on
      // the phone.
      case 'tile': {
        final action = (args['onTap'] as String?)?.trim();
        final scheme = Theme.of(context).colorScheme;
        // What the screen asked for, and a chevron where it asked for nothing
        // and there is somewhere to go. A default is a rule somebody can read;
        // ignoring the prop and guessing was neither.
        final trailing = (args['trailing'] as String?)?.trim() ??
            (action == null ? 'none' : 'chevron');
        final value = args['value'] == null ? null : _say(args['value']);
        final count = args['count'] as int?;
        final avatar = args['avatar'] == null ? null : _say(args['avatar']);

        final row = ListTile(
          // The wallet's gutter inside the group, so the words line up with
          // every other row on the phone rather than sitting on the edge.
          contentPadding: _grouped(context)
              ? const EdgeInsets.symmetric(horizontal: 16, vertical: 4)
              : null,
          // **Who the row is about, when it is about somebody.** A chat list is
          // a face and a name; an icon is a settings row. Both are rows, and
          // the difference is which of these the screen filled in.
          leading: avatar != null
              ? CircleAvatar(
                  radius: 22,
                  backgroundColor: scheme.secondaryContainer,
                  child: Text(
                    avatar.isEmpty ? '?' : avatar.characters.first.toUpperCase(),
                    style: TextStyle(color: scheme.onSecondaryContainer, fontWeight: FontWeight.w600),
                  ),
                )
              : args['icon'] == null
                  ? null
                  : Icon(_iconOf(args['icon']), size: 26, color: scheme.onSurfaceVariant),
          title: _text(style: const TextStyle(fontSize: 15, fontWeight: FontWeight.w600)),
          // The supporting line, capped at two — one long translation
          // otherwise turns a row into five lines and the list stops being
          // something anybody can scan. `TileSubtitle` in the wallet.
          subtitle: args['detail'] == null
              ? null
              : Text(
                  _detail(),
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(fontSize: 13, color: scheme.onSurfaceVariant),
                ),
          // **The right-hand side is more than one thing.** A settings row ends
          // in what it is set to, a chat row in a time and how many are
          // waiting, and either can end in a chevron. They stack in that order
          // rather than replacing each other.
          trailing: (value == null && count == null && trailing == 'none')
              ? null
              : Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    if (value != null)
                      Text(value, style: TextStyle(fontSize: 13, color: scheme.onSurfaceVariant)),
                    if (count != null) ...[
                      const SizedBox(width: 8),
                      Container(
                        constraints: const BoxConstraints(minWidth: 20),
                        padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                        decoration: BoxDecoration(
                          color: scheme.primary,
                          borderRadius: BorderRadius.circular(999),
                        ),
                        child: Text(
                          '$count',
                          textAlign: TextAlign.center,
                          style: TextStyle(fontSize: 12, color: scheme.onPrimary),
                        ),
                      ),
                    ],
                    if (trailing == 'badge' && count == null) ...[
                      const SizedBox(width: 8),
                      Container(
                        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                        decoration: BoxDecoration(
                          color: scheme.secondaryContainer,
                          borderRadius: BorderRadius.circular(999),
                        ),
                        child: Text(
                          _detail(),
                          style: TextStyle(fontSize: 12, color: scheme.onSecondaryContainer),
                        ),
                      ),
                    ],
                    if (trailing == 'chevron')
                      Icon(Icons.chevron_right, color: scheme.onSurfaceVariant),
                  ],
                ),
          onTap: action == null ? null : () => _tap(action),
        );

        // **A row inside a card does not wear a card of its own.** A settings
        // group and a chat list are rows with a rule between them under one
        // rounded edge; a card each gave eight shadows down the screen.
        return _grouped(context) ? row : Card(child: row);
      }

      case 'text':
        return _text(style: TextStyle(fontSize: 15, height: 1.55));

      case 'badge': {
        final scheme = Theme.of(context).colorScheme;
        final tone = (args['tone'] as String?)?.trim();
        final (bg, fg) = switch (tone) {
          'warning' => (scheme.tertiaryContainer, scheme.onTertiaryContainer),
          'danger' => (scheme.errorContainer, scheme.onErrorContainer),
          _ => (scheme.secondaryContainer, scheme.onSecondaryContainer),
        };
        return Align(
          alignment: Alignment.centerLeft,
          child: Container(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 3),
            decoration: BoxDecoration(color: bg, borderRadius: BorderRadius.circular(999)),
            child: _text(style: TextStyle(fontSize: 12, color: fg)),
          ),
        );
      }

      case 'avatar': {
        final scheme = Theme.of(context).colorScheme;
        final size = _sizeOf(args['size']) ?? 44;
        final initial = _say(args['of']).trim();
        return Align(
          alignment: Alignment.centerLeft,
          child: CircleAvatar(
            radius: size / 2,
            backgroundColor: scheme.secondaryContainer,
            child: Text(
              initial.isEmpty ? '?' : initial.characters.first.toUpperCase(),
              style: TextStyle(color: scheme.onSecondaryContainer, fontWeight: FontWeight.w600),
            ),
          ),
        );
      }

      // A number worth looking at, with what it is under it.
      case 'stat': {
        final scheme = Theme.of(context).colorScheme;
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              _say(args['value']),
              style: const TextStyle(fontSize: 26, fontWeight: FontWeight.w700),
            ),
            _text(style: TextStyle(fontSize: 13, color: scheme.onSurfaceVariant)),
          ],
        );
      }

      // A rule between things. In the registry from the first version and
      // never drawn, so every package that used one showed a red box saying it
      // is not in this catalogue — which it is.
      case 'divider':
        return Divider(
          height: args['style'] == 'row' ? 1 : Vaulet.lg,
          thickness: 1,
          color: Theme.of(context).colorScheme.outlineVariant,
        );

      case 'keyValue': {
        final scheme = Theme.of(context).colorScheme;
        return Padding(
          padding: const EdgeInsets.symmetric(vertical: 4),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                child: _text(style: TextStyle(fontSize: 14, color: scheme.onSurfaceVariant)),
              ),
              // **A value can be a sentence too.** `value: phrase("roleValue")`
              // arrives as the key, and printing it raw put `roleValue` on the
              // screen where the words belonged. Resolved the same way every
              // other sentence is — and a value that is not a key formats as
              // itself, which is what a number or a date is.
              Text(_say(args['value']), style: const TextStyle(fontSize: 14)),
            ],
          ),
        );
      }

      case 'link': {
        final scheme = Theme.of(context).colorScheme;
        return Align(
          alignment: Alignment.centerLeft,
          child: _text(
            style: TextStyle(
              fontSize: 15,
              color: scheme.primary,
              decoration: TextDecoration.underline,
              decorationColor: scheme.primary,
            ),
          ),
        );
      }

      case 'spinner':
        return const Padding(
          padding: EdgeInsets.symmetric(vertical: Vaulet.lg),
          child: Center(child: SizedBox(height: 22, width: 22, child: CircularProgressIndicator(strokeWidth: 2))),
        );

      // The shape of what is coming, while it is coming. Not a spinner: a
      // spinner says something is happening and a skeleton says what will be
      // there, which is what stops the screen jumping when it arrives.
      case 'skeleton': {
        final scheme = Theme.of(context).colorScheme;
        final lines = switch (args['lines']) {
          final int n when n > 0 => n,
          _ => 3,
        };
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            for (var i = 0; i < lines; i++)
              Padding(
                padding: const EdgeInsets.only(bottom: Vaulet.sm),
                child: FractionallySizedBox(
                  alignment: Alignment.centerLeft,
                  widthFactor: i == lines - 1 ? 0.6 : 1,
                  child: Container(
                    height: 12,
                    decoration: BoxDecoration(
                      color: scheme.onSurfaceVariant.withValues(alpha: 0.12),
                      borderRadius: BorderRadius.circular(6),
                    ),
                  ),
                ),
              ),
          ],
        );
      }

      // The wallet's own empty state, so an application never draws one and
      // every list that has nothing in it says so the same way.
      case 'emptyState': {
        final scheme = Theme.of(context).colorScheme;
        return Padding(
          padding: const EdgeInsets.symmetric(vertical: Vaulet.xxl),
          child: Column(
            children: [
              Icon(_iconOf(args['icon']), size: 40, color: scheme.outline),
              const SizedBox(height: Vaulet.md),
              _text(style: const TextStyle(fontSize: 15, fontWeight: FontWeight.w600)),
              if (args['detail'] != null)
                Padding(
                  padding: const EdgeInsets.only(top: 4),
                  child: Text(
                    _detail(),
                    textAlign: TextAlign.center,
                    style: TextStyle(fontSize: 13, color: scheme.onSurfaceVariant),
                  ),
                ),
            ],
          ),
        );
      }

      case 'checkbox': {
        final into = (args['into'] as String?)?.trim();
        final on = into == null ? false : _Form.of(context)[into] == true;
        return _control(
          context,
          leading: Checkbox(
            value: on,
            onChanged: into == null ? null : (v) => _Form.set(context, into, v),
          ),
          label: _text(),
          onTap: into == null ? null : () => _Form.set(context, into, !on),
        );
      }

      case 'select': {
        final into = (args['into'] as String?)?.trim();
        final options = ((args['of'] as List?) ?? const []).map((o) => '$o').toList();
        final held = into == null ? null : _Form.of(context)[into] as String?;
        // **A drawer, not a menu dropping out of the field.** The wallet asks
        // every other question of this shape in one, so a Micro App asking the
        // same question with a different object on the screen is the
        // application announcing that it is not part of the app around it.
        return _Chooser(
          label: _label(),
          value: held,
          options: options,
          icon: _iconOf(args['icon']),
          onPicked: into == null ? null : (v) => _Form.set(context, into, v),
        );
      }

      case 'slider': {
        final into = (args['into'] as String?)?.trim();
        final min = switch (args['min']) { final int n => n.toDouble(), _ => 0.0 };
        final max = switch (args['max']) { final int n => n.toDouble(), _ => 100.0 };
        final held = switch (into == null ? null : _Form.of(context)[into]) {
          final int n => n.toDouble(),
          _ => min,
        };
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _text(style: const TextStyle(fontSize: 13)),
            Slider(
              min: min,
              max: max,
              value: held.clamp(min, max),
              onChanged: into == null ? null : (v) => _Form.set(context, into, v.round()),
            ),
          ],
        );
      }

      case 'accordion':
        return Card(
          clipBehavior: Clip.antiAlias,
          child: ExpansionTile(
            title: _text(),
            subtitle: args['detail'] == null ? null : Text(_detail()),
            shape: const Border(),
            children: [
              for (final c in children)
                Padding(
                  padding: const EdgeInsets.fromLTRB(Vaulet.lg, 0, Vaulet.lg, Vaulet.md),
                  child: _Node(node: c, incoming: widget.incoming),
                ),
            ],
          ),
        );

      case 'radioGroup': {
        final into = (args['into'] as String?)?.trim();
        final options = ((args['of'] as List?) ?? const []).map((o) => '$o').toList();
        final held = into == null ? null : _Form.of(context)[into] as String?;
        // The group holds which one is chosen, rather than each button
        // carrying the answer — `groupValue` on a `Radio` is on its way out and
        // was always the same fact written once per option.
        return RadioGroup<String>(
          groupValue: held,
          onChanged: into == null ? (_) {} : (v) => _Form.set(context, into, v),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              _text(style: const TextStyle(fontSize: 13)),
              for (final o in options)
                _control(
                  context,
                  leading: Radio<String>(value: o),
                  label: Text(o, style: const TextStyle(fontSize: 15)),
                  onTap: into == null ? null : () => _Form.set(context, into, o),
                ),
            ],
          ),
        );
      }

      case 'datePicker': {
        final scheme = Theme.of(context).colorScheme;
        final into = (args['into'] as String?)?.trim();
        final held = into == null ? null : _Form.of(context)[into] as String?;
        return InkWell(
          // The wheel belongs to the wallet. An application asks for a date and
          // is handed one; it never draws a calendar of its own, which is how
          // every date in the wallet is picked the same way.
          onTap: into == null
              ? null
              : () async {
                  final picked = await showDatePicker(
                    context: context,
                    firstDate: DateTime(2000),
                    lastDate: DateTime(2100),
                    initialDate: DateTime(2026, 8, 20),
                  );
                  if (picked != null && context.mounted) {
                    _Form.set(context, into,
                        '${picked.year}-${picked.month.toString().padLeft(2, '0')}-${picked.day.toString().padLeft(2, '0')}');
                  }
                },
          child: Row(
            children: [
              SizedBox(
                width: 40,
                child: Icon(Icons.calendar_today_outlined, size: 22, color: scheme.onSurfaceVariant),
              ),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    DefaultTextStyle(
                      style: TextStyle(fontSize: 13, color: scheme.onSurfaceVariant),
                      child: _text(),
                    ),
                    Text(held ?? 'Choose a date', style: const TextStyle(fontSize: 15)),
                  ],
                ),
              ),
              Icon(Icons.chevron_right, color: scheme.onSurfaceVariant),
            ],
          ),
        );
      }

      // A table of what a list holds. Scrolls sideways on its own, because a
      // table that made the screen scroll sideways would take everything else
      // with it.
      case 'dataTable': {
        final scheme = Theme.of(context).colorScheme;
        final rows = ((args['of'] as List?) ?? const []).toList();
        final columns = ((args['columns'] as List?) ?? const []).map((c) => '$c').toList();
        if (rows.isEmpty) {
          return Padding(
            padding: const EdgeInsets.symmetric(vertical: Vaulet.xxl),
            child: Center(
              child: Text('Nothing to show',
                  style: TextStyle(color: scheme.outline, fontSize: 13)),
            ),
          );
        }
        return SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: DataTable(
            columnSpacing: Vaulet.xxl,
            headingRowHeight: 36,
            dataRowMinHeight: 36,
            dataRowMaxHeight: 44,
            columns: [
              for (final c in columns)
                DataColumn(label: Text(c, style: const TextStyle(fontSize: 12))),
            ],
            rows: [
              for (final r in rows)
                DataRow(cells: [
                  for (final c in columns)
                    DataCell(Text(
                      '${(r is Map ? r[c] : r) ?? ''}',
                      style: const TextStyle(fontSize: 13),
                    )),
                ]),
            ],
          ),
        );
      }

      case 'timeline':
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            for (final c in children) _Node(node: c, incoming: widget.incoming),
          ],
        );

      case 'timelineItem': {
        final scheme = Theme.of(context).colorScheme;
        return IntrinsicHeight(
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Column(
                children: [
                  Container(
                    margin: const EdgeInsets.only(top: 4),
                    height: 10,
                    width: 10,
                    decoration: BoxDecoration(color: scheme.primary, shape: BoxShape.circle),
                  ),
                  Expanded(child: Container(width: 1, color: scheme.outlineVariant)),
                ],
              ),
              const SizedBox(width: Vaulet.md),
              Expanded(
                child: Padding(
                  padding: const EdgeInsets.only(bottom: Vaulet.lg),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      _text(style: const TextStyle(fontSize: 15, fontWeight: FontWeight.w600)),
                      if (args['at'] != null)
                        Text('${args['at']}',
                            style: TextStyle(fontSize: 12, color: scheme.onSurfaceVariant)),
                      if (args['detail'] != null)
                        Padding(
                          padding: const EdgeInsets.only(top: 2),
                          child: Text(_detail(),
                              style: TextStyle(fontSize: 13, color: scheme.onSurfaceVariant)),
                        ),
                    ],
                  ),
                ),
              ),
            ],
          ),
        );
      }

      // The privileged three. What is drawn here is the frame the host puts
      // around them; the file, the stream and the page are the host's to fetch,
      // and the application declared the capability to have them at all.
      case 'filePicker':
      case 'video':
      case 'audio':
      case 'webContent': {
        final scheme = Theme.of(context).colorScheme;
        final kind = n['kind'] as String? ?? '';
        final (icon, what) = switch (kind) {
          'video' => (Icons.play_circle_outline, 'video'),
          'audio' => (Icons.graphic_eq, 'audio'),
          'webContent' => (Icons.public, 'page'),
          _ => (Icons.attach_file, 'file'),
        };
        return Container(
          height: kind == 'filePicker' ? 64 : 150,
          decoration: BoxDecoration(
            color: scheme.surfaceContainerHighest.withValues(alpha: 0.5),
            borderRadius: BorderRadius.circular(Vaulet.radiusCard),
            border: Border.all(color: scheme.outlineVariant),
          ),
          child: Center(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(icon, color: scheme.onSurfaceVariant),
                const SizedBox(height: 4),
                Text(
                  'the host fetches the $what',
                  style: TextStyle(fontSize: 12, color: scheme.onSurfaceVariant),
                ),
              ],
            ),
          ),
        );
      }

      // A screen's own sentence, drawn as the bar's title.
      case 'title':
        return _text(style: const TextStyle(fontSize: 20, fontWeight: FontWeight.w700));

      // The host draws its own card, for the credential the application named.
      // Nothing off the card passes through the application on the way — it
      // named a type, and the host went and found the card.
      case 'credentialCard':
      case 'idCard': {
        final of = (args['of'] as String?)?.trim() ?? '';
        final draw = _Cards.of(context);
        if (draw != null) return draw(context, of);
        // No wallet behind this renderer — a playground, or a preview. A stand
        // -in says which card would be here rather than pretending to be one.
        final scheme = Theme.of(context).colorScheme;
        return AspectRatio(
          aspectRatio: 1.586,
          child: Container(
            margin: const EdgeInsets.only(bottom: Vaulet.sm),
            decoration: BoxDecoration(
              color: scheme.surfaceContainerHighest,
              borderRadius: BorderRadius.circular(Vaulet.radiusCard),
              border: Border.all(color: scheme.outlineVariant),
            ),
            alignment: Alignment.center,
            child: Text(
              of.isEmpty ? 'a card' : of,
              style: TextStyle(fontSize: 13, color: scheme.onSurfaceVariant),
            ),
          ),
        );
      }

      case 'card': {
        // **A card of rows is a group, not a stack of cards.** A settings
        // section and a chat list are rows with a rule between them under one
        // rounded edge — which is what the wallet draws everywhere else, and
        // what a Micro App could not say: every `tile` wore a card of its own,
        // so eight rows were eight shadows down the screen.
        final rows = children.where((c) => c['kind'] == 'tile').length;
        final grouped = rows > 0 && rows == children.length;
        if (grouped) {
          final scheme = Theme.of(context).colorScheme;
          // **A list of rows is not a grey panel.** A chat list and a settings
          // screen are rows on the surface with a rule between them; the card
          // colour underneath made the whole group read as one raised block and
          // the rows inside it as text on a tint.
          final corner = _radiusOf(args['borderRadius']) ??
              BorderRadius.circular(Vaulet.radiusCard);
          // One line, drawn twice: around the group and between its rows. Two
          // weights on one object read as two objects.
          final line = scheme.outlineVariant.withValues(alpha: 0.6);
          return _Grouped(
            child: Theme(
              data: Theme.of(context).copyWith(
                splashFactory: NoSplash.splashFactory,
                highlightColor: Colors.transparent,
                splashColor: Colors.transparent,
              ),
              // **`Material`, not a painted box.** A row paints its own
              // background and its own press on the nearest `Material`
              // ancestor, so a coloured box around it hides both — Flutter
              // says so out loud, and says it on the screen that stopped
              // drawing.
              child: Material(
                color: _colorOf(args['background'], scheme) ?? scheme.surface,
                clipBehavior: Clip.antiAlias,
                shape: RoundedRectangleBorder(
                  borderRadius: corner,
                  side: BorderSide(color: line),
                ),
                child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  if (args['text'] != null)
                    Padding(
                      padding: const EdgeInsets.fromLTRB(Vaulet.lg, Vaulet.lg, Vaulet.lg, 0),
                      child: _text(style: Vaulet.cardTitle),
                    ),
                  for (var i = 0; i < children.length; i++) ...[
                    // **Edge to edge, and the same line as the frame.** An
                    // inset rule leaves a gap at both ends that reads as the
                    // group coming apart; a heavier one reads as a border
                    // inside a border.
                    if (i > 0) Divider(height: 1, thickness: 1, indent: 0, endIndent: 0, color: line),
                    _Node(node: children[i], incoming: widget.incoming),
                  ],
                ],
              ),
              ),
            ),
          );
        }
        return Card(
          child: Padding(
            padding: const EdgeInsets.all(Vaulet.lg),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                _text(style: Vaulet.cardTitle),
                for (final c in children)
                  Padding(
                    padding: const EdgeInsets.only(top: Vaulet.sm),
                    child: _Node(node: c, incoming: widget.incoming),
                  ),
              ],
            ),
          ),
        );
      }

      // The wallet's own home screen leads with these, and a Micro App that
      // wanted one had to build it out of a card that held nothing.
      case 'banner':
        final action = (args['onTap'] as String?)?.trim();
        final scheme = Theme.of(context).colorScheme;
        return Card(
          // The pager runs to the edge; the banner keeps the screen's own
          // margin, so a page turning slides one card past another rather than
          // one full-bleed rectangle past another.
          margin: const EdgeInsets.symmetric(horizontal: kScreenPadH),
          clipBehavior: Clip.antiAlias,
          child: InkWell(
            onTap: action == null ? null : () => _tap(action),
            child: SizedBox(
              height: 132,
              child: Stack(
                fit: StackFit.expand,
                children: [
                  Container(color: scheme.secondaryContainer),
                  Align(
                    alignment: Alignment.centerRight,
                    child: Padding(
                      padding: const EdgeInsets.only(right: Vaulet.lg),
                      child: Icon(
                        _iconOf(args['icon']),
                        size: 72,
                        color: scheme.onSecondaryContainer.withValues(alpha: 0.25),
                      ),
                    ),
                  ),
                  Padding(
                    padding: const EdgeInsets.all(Vaulet.lg),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      mainAxisAlignment: MainAxisAlignment.end,
                      children: [
                        _text(
                          style: TextStyle(
                            fontSize: 19,
                            fontWeight: FontWeight.w700,
                            color: scheme.onSecondaryContainer,
                          ),
                        ),
                        if (args['detail'] != null)
                          Padding(
                            padding: const EdgeInsets.only(top: 4),
                            child: Text(
                              _detail(),
                              maxLines: 2,
                              overflow: TextOverflow.ellipsis,
                              style: TextStyle(
                                fontSize: 13,
                                color: scheme.onSecondaryContainer.withValues(alpha: 0.8),
                              ),
                            ),
                          ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
        );

      // Across and down. The wallet's own home screen puts its mini apps in
      // one; a screen of reward tiles or categories wants the same shape.
      case 'grid':
        final columns = switch (args['columns']) {
          final int n when n > 0 => n,
          _ => 2,
        };
        return GridView.count(
          crossAxisCount: columns,
          shrinkWrap: true,
          // The screen scrolls; a grid inside it that scrolled as well would be
          // two scrollbars for one gesture.
          physics: const NeverScrollableScrollPhysics(),
          crossAxisSpacing: Vaulet.sm,
          mainAxisSpacing: Vaulet.sm,
          childAspectRatio: 1.1,
          children: [
            for (final c in children) _Node(node: c, incoming: widget.incoming),
          ],
        );

      // Sideways, with the dots the wallet's own home screen has.
      case 'carousel':
        return _Carousel(pages: children, incoming: widget.incoming);

      case 'list':
        // Already expanded: one child per row, each with its slots resolved.
        // An empty list is the host's empty state — the application never
        // draws one, and there is nothing here for it to draw with.
        final rows = children;
        if (rows.isEmpty) {
          return Padding(
            padding: const EdgeInsets.symmetric(vertical: Vaulet.xxl),
            child: Center(
              child: Text('Nothing here yet',
                  style: TextStyle(color: Theme.of(context).colorScheme.outline, fontSize: 13)),
            ),
          );
        }
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            for (final row in rows)
              Padding(
                padding: const EdgeInsets.only(bottom: Vaulet.sm),
                child: _Node(node: row, incoming: widget.incoming),
              ),
          ],
        );

      // What a person types, held by the host until they submit. `into` names
      // the input it becomes.
      case 'field': {
        final into = (args['into'] as String?)?.trim();
        final kind = (args['kind'] as String?)?.trim();
        final scheme = Theme.of(context).colorScheme;
        // **A box somebody types in looks like a box.** It was a line of text
        // with an icon beside it and no edge at all, which reads as something
        // already filled in rather than something to fill in. The frame is the
        // one the chooser next door wears, so the two questions on one screen
        // are the same object.
        return Container(
          padding: const EdgeInsets.fromLTRB(14, 6, 12, 6),
          decoration: BoxDecoration(
            color: scheme.surface,
            border: Border.all(color: scheme.outlineVariant),
            borderRadius: BorderRadius.circular(Vaulet.radiusCard),
          ),
          child: Row(
            // Top, not centre: a field that has grown to three lines is still
            // one row about one thing, and an icon that slid to the middle
            // points at the second line of it.
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              if (args['icon'] != null) ...[
                SizedBox(
                  height: 44,
                  child: Center(
                    child: Icon(_iconOf(args['icon']), size: 20, color: scheme.onSurfaceVariant),
                  ),
                ),
                const SizedBox(width: 10),
              ],
              Expanded(
                child: TextField(
                  keyboardType: kind == 'number' ? TextInputType.number : TextInputType.text,
                  style: const TextStyle(fontSize: 15, fontWeight: FontWeight.w400),
                  decoration: InputDecoration(
                    labelText: _label(),
                    labelStyle: TextStyle(fontSize: 12, color: scheme.onSurfaceVariant),
                    floatingLabelStyle: TextStyle(fontSize: 12, color: scheme.onSurfaceVariant),
                    // Always floating: the label names the box whether or not
                    // there is anything in it, and a label that moves on focus
                    // is a label somebody has to have already read.
                    floatingLabelBehavior: FloatingLabelBehavior.always,
                    border: InputBorder.none,
                    isDense: true,
                    contentPadding: const EdgeInsets.symmetric(vertical: 4),
                  ),
                  onChanged: into == null ? null : (v) => _Form.set(context, into, v),
                ),
              ),
            ],
          ),
        );
      }

      case 'toggle': {
        final into = (args['into'] as String?)?.trim();
        final on = into == null ? false : _Form.of(context)[into] == true;
        return _control(
          context,
          label: _text(),
          control: Switch(
            value: on,
            onChanged: into == null ? null : (v) => _Form.set(context, into, v),
          ),
          onTap: into == null ? null : () => _Form.set(context, into, !on),
        );
      }

      case 'button':
        final emphasis = (args['emphasis'] as String?)?.trim();
        final action = (args['onTap'] as String?)?.trim();
        final state = (args['state'] as String?)?.trim();
        // A button that is working says so, and one that cannot be pressed does
        // not look pressable. An action through the runner takes seconds, and a
        // button that looked idle through all of them was the screen lying.
        final busy = state == 'busy';
        final child = busy
            ? const SizedBox(
                height: 16,
                width: 16,
                child: CircularProgressIndicator(strokeWidth: 2),
              )
            : _text(style: const TextStyle(fontSize: 15, fontWeight: FontWeight.w600));
        final press = (state == 'disabled' || busy) ? null : () => _tap(action);
        return Tooltip(
          message: action == null
              ? ''
              : 'calls $action, through require → verify → compute → update → execute',
          child: emphasis == 'primary'
              ? FilledButton(onPressed: press, child: child)
              : OutlinedButton(onPressed: press, child: child),
        );

      // Not a component this host ships. Drawing something approximate would be
      // the preview inventing a catalogue — which is the whole mistake it exists
      // to stop, and the thing a version number is for.
      default:
        return Padding(
          padding: const EdgeInsets.only(bottom: Vaulet.sm),
          child: Container(
            padding: const EdgeInsets.all(Vaulet.md),
            decoration: BoxDecoration(
              color: Theme.of(context).colorScheme.errorContainer,
              borderRadius: BorderRadius.circular(Vaulet.radiusCard),
            ),
            child: Text(
              '`${n['kind']}` is not in this catalogue',
              style: TextStyle(fontSize: 12, color: Theme.of(context).colorScheme.onErrorContainer),
            ),
          ),
        );
    }
  }

  /// A press carries the form with it.
  ///
  /// What a field holds while somebody is typing belongs to the host — it is
  /// not application state, it is not hashed and it is not in the record. The
  /// action is given what the form held at the moment it was submitted, which
  /// is what `input` is.
  void _tap(String? target) {
    if (target == null) return;
    // `onTap: Detail` is `navigation.navigate(to: Detail)` written short, and
    // moving between screens is the host's own business — it never reaches the
    // application, which is why nothing is posted for it.
    final with_ = (args['onTapWith'] as Map?)?.cast<String, Object?>() ?? const <String, Object?>{};
    final nav = _Nav.of(context);
    if (target == 'navigation.back') {
      // What a screen returns goes into the form of the one that opened it.
      nav?.back((with_['with'] as Map?)?.cast<String, Object?>() ?? with_);
      return;
    }
    if (nav != null && nav.go(target, with_)) return;

    // Everything else was the host's own business. This is the application's:
    // a press it declared, with what the form on this screen holds.
    nav?.run(target, _Form.of(context));
  }


}

/// One of a few things, chosen in a drawer.
///
/// **Framed like the wallet's own inputs**: a box with a border, the question
/// small above the answer, and a chevron saying it opens. A row of text with no
/// edge reads as something already filled in.
class _Chooser extends StatelessWidget {
  const _Chooser({
    required this.label,
    required this.value,
    required this.options,
    required this.icon,
    required this.onPicked,
  });

  final String label;
  final String? value;
  final List<String> options;
  final IconData? icon;
  final void Function(String)? onPicked;

  Future<void> _open(BuildContext context) async {
    final host = _Cards.chooser(context);
    final picked = host != null
        ? await host(context, label, options)
        : await showModalBottomSheet<String>(
            context: context,
            useSafeArea: true,
            backgroundColor: Colors.transparent,
            builder: (sheet) => _OptionSheet(title: label, options: options),
          );
    if (picked != null) onPicked?.call(picked);
  }

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final chosen = value != null && value!.isNotEmpty;
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: onPicked == null ? null : () => _open(context),
      child: Container(
        padding: const EdgeInsets.fromLTRB(14, 10, 12, 10),
        decoration: BoxDecoration(
          color: scheme.surface,
          border: Border.all(color: scheme.outlineVariant),
          borderRadius: BorderRadius.circular(Vaulet.radiusCard),
        ),
        child: Row(
          children: [
            if (icon != null) ...[
              Icon(icon, size: 20, color: scheme.onSurfaceVariant),
              const SizedBox(width: 10),
            ],
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(label,
                      style: TextStyle(fontSize: 12, color: scheme.onSurfaceVariant)),
                  const SizedBox(height: 2),
                  Text(
                    chosen ? value! : '—',
                    style: TextStyle(
                      fontSize: 15,
                      color: chosen ? scheme.onSurface : scheme.onSurfaceVariant,
                    ),
                  ),
                ],
              ),
            ),
            Icon(Icons.expand_more, size: 20, color: scheme.onSurfaceVariant),
          ],
        ),
      ),
    );
  }
}

/// The renderer's own drawer, for a host that provides none — a playground, a
/// preview. A wallet passes `chooser` and this is never seen.
class _OptionSheet extends StatelessWidget {
  const _OptionSheet({required this.title, required this.options});

  final String title;
  final List<String> options;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      decoration: BoxDecoration(
        color: scheme.surface,
        borderRadius: const BorderRadius.vertical(top: Radius.circular(20)),
      ),
      clipBehavior: Clip.antiAlias,
      child: SafeArea(
        top: false,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Container(
              alignment: Alignment.center,
              padding: const EdgeInsets.symmetric(vertical: 10),
              child: Container(
                width: 36,
                height: 4,
                decoration: BoxDecoration(
                  color: scheme.onSurfaceVariant.withValues(alpha: 0.4),
                  borderRadius: BorderRadius.circular(2),
                ),
              ),
            ),
            if (title.isNotEmpty)
              Padding(
                padding: const EdgeInsets.fromLTRB(24, 4, 24, 8),
                child: Align(
                  alignment: Alignment.centerLeft,
                  child: Text(title,
                      style: const TextStyle(fontSize: 20, fontWeight: FontWeight.w700)),
                ),
              ),
            for (final o in options)
              ListTile(
                title: Text(o),
                onTap: () => Navigator.of(context).pop(o),
              ),
            const SizedBox(height: 8),
          ],
        ),
      ),
    );
  }
}

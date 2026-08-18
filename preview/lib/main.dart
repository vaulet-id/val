// The preview, drawn by the host's own toolkit.
//
// This is not a picture of what a VAL screen would look like. It is the
// catalogue: `button` is a Flutter button, `tabs` is a Flutter TabBar, and the
// text is laid out by the same engine that will lay it out on a phone. A
// facsimile in HTML would have agreed with itself and been wrong about
// wrapping, about Thai line breaking, and about every metric that matters.
//
// The application supplies structure and keys. Everything visual — spacing,
// colour, type, the empty state, which tab is open — belongs here, which is the
// division the specification draws and the reason a screen can be trusted.

import 'dart:convert';
import 'dart:js_interop';

import 'package:flutter/material.dart';
import 'package:web/web.dart' as web;

void main() => runApp(const PreviewApp());

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

class PreviewApp extends StatefulWidget {
  const PreviewApp({super.key});

  @override
  State<PreviewApp> createState() => _PreviewAppState();
}

class _PreviewAppState extends State<PreviewApp> {
  Incoming _in = Incoming.empty;

  @override
  void initState() {
    super.initState();
    web.window.addEventListener(
      'message',
      ((web.MessageEvent e) {
        final data = e.data.dartify();
        if (data is! String) return;
        try {
          setState(() => _in = Incoming.fromJson(jsonDecode(data) as Map<String, dynamic>));
        } catch (_) {
          // A malformed message is the playground's problem, not something to
          // draw an error screen about.
        }
      }).toJS,
    );
    // Tell the parent we are ready, so it does not post into a blank frame.
    web.window.parent?.postMessage('{"ready":true}'.toJS, '*'.toJS);
  }

  @override
  /// Which screens have been opened, deepest last. The host owns this: an
  /// application declares where a press goes and never how the stack behaves,
  /// which is why there is no push, pop or replace for it to get wrong.
  final List<String> _stack = [];

  Map<String, dynamic> get _current {
    final byName = {
      for (final s in _in.screens) (s as Map<String, dynamic>)['name'] as String: s,
    };
    for (final name in _stack.reversed) {
      final found = byName[name];
      if (found != null) return found as Map<String, dynamic>;
    }
    return _opening;
  }

  /// Where the package opens: the screen that says so, or the only one there is.
  Map<String, dynamic> get _opening {
    final screens = _in.screens.cast<Map<String, dynamic>>();
    return screens.firstWhere(
      (s) => s['start'] == true,
      orElse: () => screens.first,
    );
  }

  /// Whether a press names a screen rather than an action. The host knows its
  /// own screens, so nothing has to be marked on the way out.
  bool _isScreen(String target) =>
      _in.screens.any((s) => (s as Map<String, dynamic>)['name'] == target);

  void _navigate(String screen, [Map<String, Object?> args = const {}]) {
    setState(() {
      if (_stack.isEmpty) _stack.add(_opening['name'] as String);
      _stack.add(screen);
    });
    // A screen that takes parameters cannot have been resolved ahead of time —
    // its content depends on the row that opened it — so the host is asked for
    // it with what the press handed over.
    if (args.isNotEmpty) {
      web.window.parent?.postMessage(
        jsonEncode({'type': 'screen', 'screen': screen, 'args': args}).toJS,
        '*'.toJS,
      );
    }
  }

  void _back() => setState(() {
        if (_stack.length > 1) _stack.removeLast();
      });

  Widget build(BuildContext context) {
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      theme: Vaulet.theme(_in.dark ? Brightness.dark : Brightness.light),
      home: Scaffold(
        backgroundColor: Colors.transparent,
        body: _in.screens.isEmpty
            ? const _NoScreen()
            : Padding(
                // Room around the device so it reads as an object on a surface
                // rather than a panel that ran out of space.
                padding: const EdgeInsets.symmetric(horizontal: Vaulet.xxl, vertical: 24),
                child: Center(
                  // Contained, not stretched, and fitted to the *height* as
                  // well: a device preview that runs off the bottom of its
                  // panel is one you have to scroll to find the docked action,
                  // which is the one thing its position was supposed to prove.
                  child: FittedBox(
                    fit: BoxFit.contain,
                    child: _Phone(
                      screen: _current,
                      incoming: _in,
                      canGoBack: _stack.length > 1,
                      onNavigate: _navigate,
                      onBack: _back,
                    ),
                  ),
                ),
              ),
      ),
    );
  }
}

class _NoScreen extends StatelessWidget {
  const _NoScreen();

  @override
  Widget build(BuildContext context) => Center(
    child: Padding(
      padding: const EdgeInsets.all(24),
      child: Text(
        'This program declares no screen.\n'
        'An application can be actions, trust policies and state — '
        'the loyalty card was, before it had one.',
        textAlign: TextAlign.center,
        style: Theme.of(context).textTheme.bodySmall,
      ),
    ),
  );
}

/// The frame is the host's, and so is everything inside it.
///
/// The size is an iPhone's, not a rectangle that looked about right: **393 × 852
/// logical points**, which is what an iPhone 15 and 16 Pro hand a Flutter app.
/// A preview at some other size is a preview that will not tell you the one
/// thing it is for — whether the Thai wraps, whether the row fits, whether the
/// button is reachable with a thumb.
class _Phone extends StatelessWidget {
  const _Phone({
    required this.screen,
    required this.incoming,
    this.canGoBack = false,
    this.onNavigate,
    this.onBack,
  });

  final bool canGoBack;
  final void Function(String, [Map<String, Object?>])? onNavigate;
  final VoidCallback? onBack;

  static const width = 393.0;
  static const height = 852.0;

  /// The screen's corner, and the bezel around it. iPhone corners are a
  /// continuous curve rather than a circular one; `BorderRadius` is close
  /// enough at this size and the difference is not what anybody is here to
  /// check.
  static const screenRadius = 47.0;
  static const bezel = 12.0;

  final Map<String, dynamic> screen;
  final Incoming incoming;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;

    // Only the phone. What this screen sees — which credentials, under which
    // policy, and which of them anybody signed — is in the report, where it is
    // derived from the code rather than drawn beside it. A preview that carried
    // its own copy would be a second answer to a question that has one.
    //
    // Scaled to whatever the panel gives it, never stretched: a preview that
    // changed the aspect ratio to fit would be answering a question nobody
    // asked.
    return Center(
      child: FittedBox(
        child: Container(
          width: width + bezel * 2,
          height: height + bezel * 2,
          decoration: BoxDecoration(
            color: const Color(0xFF1C1C1E),
            borderRadius: BorderRadius.circular(screenRadius + bezel),
            boxShadow: [
              BoxShadow(
                color: Colors.black.withValues(alpha: 0.35),
                blurRadius: 24,
                offset: const Offset(0, 8),
              ),
            ],
          ),
          padding: const EdgeInsets.all(bezel),
          child: ClipRRect(
            borderRadius: BorderRadius.circular(screenRadius),
            child: Container(
              width: width,
              height: height,
              color: scheme.surface,
              child: Column(
                children: [
                  const _StatusBar(),
                  Expanded(
                    child: _Screen(
                      screen: screen,
                      incoming: incoming,
                      canGoBack: canGoBack,
                      onNavigate: onNavigate,
                      onBack: onBack,
                    ),
                  ),
                  const _HomeIndicator(),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// Where a press can go, for the nodes deep in a screen.
///
/// Moving between screens is the host's: it never reaches the application, so
/// nothing about it is posted out and nothing about it is in the record.
class _Nav extends InheritedWidget {
  const _Nav({required this.go, required super.child});

  /// True when the target was a screen and the move happened.
  final bool Function(String, Map<String, Object?>) go;

  static bool Function(String, Map<String, Object?>)? of(BuildContext c) =>
      c.dependOnInheritedWidgetOfExactType<_Nav>()?.go;

  @override
  bool updateShouldNotify(_Nav old) => false;
}

/// What the form holds, until it is submitted.
///
/// The host's, not the application's: a scroll position or a half-typed field in
/// application state would be hashed, signed and replayed, and "provable" would
/// mean less by one press each time.
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
    this.canGoBack = false,
    this.onNavigate,
    this.onBack,
  });

  final Map<String, dynamic> screen;
  final Incoming incoming;
  final bool canGoBack;
  final void Function(String, [Map<String, Object?>])? onNavigate;
  final VoidCallback? onBack;

  @override
  State<_Screen> createState() => _ScreenState();
}

class _ScreenState extends State<_Screen> {
  final Map<String, Object?> _values = {};

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

    return _Nav(
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
      body: ListView(
        padding: const EdgeInsets.fromLTRB(kScreenPadH, Vaulet.sm, kScreenPadH, Vaulet.lg),
        children: [
          for (final n in _prune(nodes)) _Node(node: n, incoming: widget.incoming),
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

/// The wallet's standard horizontal gutter.
const double kScreenPadH = 16;

/// Status bar and Dynamic Island. Drawn because they take space a screen does
/// not get to use, and a preview that ignored them would show a layout that
/// fits when the real one does not.
class _StatusBar extends StatelessWidget {
  const _StatusBar();

  @override
  Widget build(BuildContext context) {
    final on = Theme.of(context).colorScheme.onSurface;
    return SizedBox(
      height: 59,
      child: Stack(
        alignment: Alignment.center,
        children: [
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 28),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              crossAxisAlignment: CrossAxisAlignment.center,
              children: [
                Text(
                  '9:41',
                  style: TextStyle(color: on, fontSize: 15, fontWeight: FontWeight.w600),
                ),
                Row(
                  children: [
                    Icon(Icons.signal_cellular_alt, size: 15, color: on),
                    const SizedBox(width: 4),
                    Icon(Icons.wifi, size: 15, color: on),
                    const SizedBox(width: 4),
                    Icon(Icons.battery_full, size: 16, color: on),
                  ],
                ),
              ],
            ),
          ),
          Align(
            alignment: const Alignment(0, -0.15),
            child: Container(
              width: 125,
              height: 36,
              decoration: BoxDecoration(
                color: Colors.black,
                borderRadius: BorderRadius.circular(18),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _HomeIndicator extends StatelessWidget {
  const _HomeIndicator();

  @override
  Widget build(BuildContext context) => SizedBox(
    height: 34,
    child: Center(
      child: Container(
        width: 139,
        height: 5,
        decoration: BoxDecoration(
          color: Theme.of(context).colorScheme.onSurface.withValues(alpha: 0.85),
          borderRadius: BorderRadius.circular(3),
        ),
      ),
    ),
  );
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
    final template = entry?[widget.incoming.locale] as String?;
    return template ?? key;
  }

  Widget _text({TextStyle? style, bool upper = false}) {
    final key = args['text'] as String?;
    if (key == null) return const SizedBox.shrink();
    final entry = (widget.incoming.text[key] as Map?)?.cast<String, dynamic>();
    if (entry == null) {
      return Text(
        'missing key “$key”',
        style: TextStyle(color: Theme.of(context).colorScheme.error, fontSize: 12),
      );
    }
    var template = entry[widget.incoming.locale] as String?;
    if (upper) template = template?.toUpperCase();
    if (template == null) {
      return Text(
        '“$key” has no ${widget.incoming.locale}',
        style: TextStyle(color: Theme.of(context).colorScheme.error, fontSize: 12),
      );
    }

    final spans = <InlineSpan>[];
    final pattern = RegExp(r'\{([a-zA-Z_]+)\}');
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
    return Text.rich(TextSpan(children: spans), style: style ?? const TextStyle(fontSize: 13));
  }

  @override
  Widget build(BuildContext context) {
    switch (n['kind'] as String? ?? '') {
      case 'column':
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            for (final c in children)
              Padding(
                padding: const EdgeInsets.only(bottom: Vaulet.sm),
                child: _Node(node: c, incoming: widget.incoming),
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
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              for (final child in children)
                Expanded(child: _Node(node: child, incoming: widget.incoming)),
            ],
          ),
        );

      // `AppCard` + `ListTile`: the leading icon, the title, a subtitle capped
      // at two lines, and a chevron when a press goes somewhere. A bare row of
      // text would put the title in a different place from every other row on
      // the phone.
      case 'tile':
        final action = (args['onTap'] as String?)?.trim();
        return Card(
          child: ListTile(
            leading: Icon(Icons.receipt_long_outlined, size: 28,
                color: Theme.of(context).colorScheme.onSurfaceVariant),
            title: _text(),
            trailing: action == null
                ? null
                : Icon(Icons.chevron_right, color: Theme.of(context).colorScheme.onSurfaceVariant),
            onTap: action == null ? null : () => _tap(action),
          ),
        );

      // A screen's own sentence, drawn as the bar's title.
      case 'title':
        return _text(style: const TextStyle(fontSize: 20, fontWeight: FontWeight.w700));

      case 'card':
        return Card(
          child: Padding(
            padding: const EdgeInsets.all(Vaulet.lg),
            child: _text(style: Vaulet.cardTitle),
          ),
        );

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
      case 'field':
        final into = (args['into'] as String?)?.trim();
        final kind = (args['kind'] as String?)?.trim();
        return Padding(
          padding: const EdgeInsets.symmetric(vertical: Vaulet.sm),
          child: TextField(
            keyboardType: kind == 'number' ? TextInputType.number : TextInputType.text,
            decoration: InputDecoration(
              labelText: _label(),
              border: const OutlineInputBorder(),
              isDense: true,
            ),
            onChanged: into == null ? null : (v) => _Form.set(context, into, v),
          ),
        );

      case 'toggle':
        final into = (args['into'] as String?)?.trim();
        return SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: _text(),
          value: into == null ? false : _Form.of(context)[into] == true,
          onChanged: into == null
              ? null
              : (v) => _Form.set(context, into, v),
        );

      case 'button':
        final emphasis = (args['emphasis'] as String?)?.trim();
        final action = (args['onTap'] as String?)?.trim();
        final child = _text(style: const TextStyle(fontSize: 15, fontWeight: FontWeight.w600));
        return Tooltip(
          message: action == null
              ? ''
              : 'calls $action, through require → verify → compute → update → execute',
          child: emphasis == 'primary'
              ? FilledButton(onPressed: () => _tap(action), child: child)
              : OutlinedButton(onPressed: () => _tap(action), child: child),
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
    final navigate = _Nav.of(context);
    final with_ = (args['onTapWith'] as Map?)?.cast<String, Object?>() ?? const <String, Object?>{};
    if (navigate != null && navigate(target, with_)) return;

    web.window.parent?.postMessage(
      jsonEncode({'type': 'tap', 'action': target, 'input': _Form.of(context)}).toJS,
      '*'.toJS,
    );
  }


}

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

/// A wallet with nothing in it that anybody signed.
///
/// The preview is a host, so the wallet belongs here rather than in the
/// playground: resolving what a screen declared is the host's half of the
/// division, and putting it on the other side would have been a second
/// evaluator to keep faithful to the first.
///
/// Everything below is invented. It is shaped like the declarations in the
/// examples and stands in for a phone that has a membership and a few receipts
/// on it. **No issuer is behind any of it**, which the panel says out loud —
/// a preview that let mock data look issuer-backed would be teaching the one
/// habit this whole system exists to break.
class MockHost {
  MockHost._();

  /// The fallback, for a frame nobody has posted into yet. The real wallet
  /// arrives with the screen — `fixtures/wallet.json`, which somebody can edit.
  static const state = <String, Object?>{
    'lifetimePoints': 1365,
    'member': <String, Object?>{
      'member_id': 'M-2891',
      'points': 1365,
      'tier': 'bronze',
    },
  };

  /// What `credentials of PurchaseReceipt verified with ReceiptFromMerchant`
  /// resolves to. A real host would have checked the policy before handing
  /// these over; this one pretends it did.
  static const receipts = <Map<String, Object?>>[
    {'merchant': 'Codefin Coffee', 'amount': 12500, 'purchased_at': '2026-08-16T09:12:00Z'},
    {'merchant': 'Siam Bookshop', 'amount': 48000, 'purchased_at': '2026-08-14T18:40:00Z'},
    {'merchant': 'Ari Market', 'amount': 6900, 'purchased_at': '2026-08-11T07:05:00Z'},
  ];

  /// How many rows a `list` draws. The declaration's `limit` bounds it; this
  /// host simply has three.
  static int get rows => receipts.length;

  /// Resolve one slot expression — `state.member.points`, `r.claims.merchant`.
  ///
  /// Deliberately not an evaluator. It walks a path and nothing else: an
  /// expression this cannot follow comes back as itself, because a preview that
  /// guessed at arithmetic would be a third implementation of the language and
  /// the first one nobody tests.
  static Object? slot(
    String expr, {
    required Incoming wallet,
    String? bind,
    Map<String, Object?>? item,
  }) {
    final parts = expr.split('.');
    if (parts.isEmpty) return null;

    Object? cursor;
    var rest = parts;
    if (parts.first == 'state' || parts.first == 'next') {
      cursor = wallet.state;
      rest = parts.sublist(1);
    } else if (bind != null && parts.first == bind) {
      cursor = item;
      // `r.claims.merchant` — a credential's claims are one hop the host knows
      // about, because the host is what handed the credential over.
      rest = parts.sublist(1);
      if (rest.isNotEmpty && rest.first == 'claims') rest = rest.sublist(1);
    } else {
      return null;
    }

    for (final key in rest) {
      if (cursor is Map<String, Object?>) {
        cursor = cursor[key];
      } else {
        return null;
      }
    }
    return cursor;
  }

  /// The host formats. An application that formatted a number would get the
  /// thousands separator, the era and the currency position wrong separately
  /// from every other application — so it never touches one.
  static String format(Object? value, String locale) {
    if (value == null) return '—';
    if (value is int) {
      final digits = value.abs().toString();
      final out = StringBuffer(value < 0 ? '-' : '');
      for (var i = 0; i < digits.length; i++) {
        if (i > 0 && (digits.length - i) % 3 == 0) out.write(',');
        out.write(digits[i]);
      }
      return out.toString();
    }
    if (value is String) {
      final at = DateTime.tryParse(value);
      if (at == null) return value;
      const months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
      const thai = ['ม.ค.', 'ก.พ.', 'มี.ค.', 'เม.ย.', 'พ.ค.', 'มิ.ย.', 'ก.ค.', 'ส.ค.', 'ก.ย.', 'ต.ค.', 'พ.ย.', 'ธ.ค.'];
      // Buddhist era in Thai, which is the sort of thing that is wrong in forty
      // applications the moment forty applications are allowed to do it.
      return locale == 'th'
          ? '${at.day} ${thai[at.month - 1]} ${at.year + 543}'
          : '${at.day} ${months[at.month - 1]} ${at.year}';
    }
    return value.toString();
  }
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
  static ThemeData theme(Brightness brightness) {
    var scheme = ColorScheme.fromSeed(seedColor: seed, brightness: brightness);
    if (brightness == Brightness.light) {
      scheme = scheme.copyWith(primary: seed, onPrimary: Colors.white);
    }
    final buttonShape = RoundedRectangleBorder(borderRadius: BorderRadius.circular(radiusCard));
    return ThemeData(
      useMaterial3: true,
      colorScheme: scheme,
      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(
          minimumSize: const Size.fromHeight(buttonHeight),
          textStyle: const TextStyle(fontSize: 15, fontWeight: FontWeight.w600),
          shape: buttonShape,
        ),
      ),
      outlinedButtonTheme: OutlinedButtonThemeData(
        style: OutlinedButton.styleFrom(
          minimumSize: const Size.fromHeight(buttonHeight),
          textStyle: const TextStyle(fontSize: 15, fontWeight: FontWeight.w600),
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
    required this.wallet,
    required this.locale,
    required this.dark,
  });

  final List<dynamic> screens;
  final Map<String, dynamic> text;

  /// The host's own data. Editing it in the playground changes what this draws
  /// without changing a line of the application — which is what "the data is
  /// the host's" means, shown rather than argued.
  final Map<String, dynamic> wallet;
  final String locale;
  final bool dark;

  static const empty = Incoming(
    screens: [],
    text: {},
    wallet: {},
    locale: 'en',
    dark: false,
  );

  factory Incoming.fromJson(Map<String, dynamic> j) => Incoming(
    screens: (j['screens'] as List?) ?? const [],
    text: (j['text'] as Map?)?.cast<String, dynamic>() ?? const {},
    wallet: (j['wallet'] as Map?)?.cast<String, dynamic>() ?? const {},
    locale: (j['locale'] as String?) ?? 'en',
    dark: (j['dark'] as bool?) ?? false,
  );

  Map<String, Object?> get state =>
      (wallet['state'] as Map?)?.cast<String, Object?>() ?? MockHost.state;

  List<Map<String, Object?>> rowsOf(String type) {
    final rows = (wallet['credentials'] as Map?)?[type]?['rows'] as List?;
    return rows?.cast<Map<String, Object?>>() ?? const [];
  }

  /// `list(receipts)` names a binding; the screen's `data` block says what that
  /// binding resolves to. The host looks it up, because the host is what
  /// resolved it in the first place.
  String typeFor(String binding) {
    for (final s in screens) {
      for (final d in ((s as Map)['data'] as List? ?? const [])) {
        if ((d as Map)['name'] == binding) return (d['type'] as String?) ?? '';
      }
    }
    return '';
  }
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
  Widget build(BuildContext context) {
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      theme: Vaulet.theme(_in.dark ? Brightness.dark : Brightness.light),
      home: Scaffold(
        backgroundColor: Colors.transparent,
        body: _in.screens.isEmpty
            ? const _NoScreen()
            : ListView(
                // Room around the device so it reads as an object on a surface
                // rather than as a panel that ran out of room — and enough of
                // it above that the phone is not touching the tab bar it sits
                // under.
                padding: const EdgeInsets.symmetric(horizontal: Vaulet.xxl, vertical: 40),
                children: [
                  for (final s in _in.screens)
                    _Phone(screen: s as Map<String, dynamic>, incoming: _in),
                ],
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
  const _Phone({required this.screen, required this.incoming});

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
                  Expanded(child: _Screen(screen: screen, incoming: incoming)),
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

/// A screen, built the way the wallet builds one: an `AppBar`, a scrolling body
/// on the standard 16pt gutter, and the primary action **docked at the bottom**
/// rather than sitting inline in the content.
///
/// The last one is a layout fact rather than a preference. A screen whose call
/// to action scrolls with the content is a screen where it is sometimes not on
/// screen, and the wallet answered that once — in `BottomActionBar`, for every
/// screen — so a preview that put the button inline would be showing a layout
/// the host does not produce.
class _Screen extends StatelessWidget {
  const _Screen({required this.screen, required this.incoming});

  final Map<String, dynamic> screen;
  final Incoming incoming;

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
    final nodes = ((screen['tree'] as List?) ?? const []).cast<Map<String, dynamic>>();
    final docked = _flatten(nodes).where(_isPrimary).toList();

    return Scaffold(
      backgroundColor: Theme.of(context).colorScheme.surface,
      appBar: AppBar(
        toolbarHeight: 52,
        title: Text(
          screen['name'] as String? ?? '',
          style: const TextStyle(fontSize: 20, fontWeight: FontWeight.w700),
        ),
      ),
      body: ListView(
        padding: const EdgeInsets.fromLTRB(kScreenPadH, Vaulet.sm, kScreenPadH, Vaulet.lg),
        children: [
          for (final n in _prune(nodes)) _Node(node: n, incoming: incoming),
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
                    SizedBox(width: double.infinity, child: _Node(node: b, incoming: incoming)),
                ],
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
  const _Node({required this.node, required this.incoming, this.bind, this.item});

  final Map<String, dynamic> node;
  final Incoming incoming;

  /// `list(receipts) { r -> … }` — what `r` is, and which row this is.
  final String? bind;
  final Map<String, Object?>? item;

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
      final expr = args[m.group(1)] as String?;
      // The host resolves the slot against its own wallet and formats it. An
      // application that formatted a number would get the thousands separator,
      // the era and the currency position wrong separately from every other
      // application — so it never touches one.
      final value = expr == null
          ? null
          : MockHost.slot(
              expr,
              wallet: widget.incoming,
              bind: widget.bind,
              item: widget.item,
            );
      spans.add(
        WidgetSpan(
          alignment: PlaceholderAlignment.baseline,
          baseline: TextBaseline.alphabetic,
          child: Tooltip(
            message: expr ?? '',
            child: Text(
              value == null
                  ? '${m.group(1)}?'
                  : MockHost.format(value, widget.incoming.locale),
              style: TextStyle(
                fontWeight: FontWeight.w600,
                color: value == null
                    ? Theme.of(context).colorScheme.error
                    : null,
              ),
            ),
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

      // `AppCard` + `ListTile`: the leading icon, the title, a subtitle capped
      // at two lines, and a chevron when a press goes somewhere. A bare row of
      // text would put the title in a different place from every other row on
      // the phone.
      case 'row':
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

      case 'card':
        return Card(
          child: Padding(
            padding: const EdgeInsets.all(Vaulet.lg),
            child: _text(style: Vaulet.cardTitle),
          ),
        );

      case 'list':
        final row = children.isEmpty ? null : children.first;
        // The rows the host handed back, not three copies of one. An empty list
        // is the host's empty state — the application never draws one.
        final type = widget.incoming.typeFor((args['0'] as String?) ?? '');
        final rows = widget.incoming.rowsOf(type);
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
            for (final item in rows)
              Padding(
                padding: const EdgeInsets.only(bottom: Vaulet.sm),
                child: row == null
                    ? const Card(child: ListTile(dense: true, title: Text('row')))
                    : _Node(
                        node: row,
                        incoming: widget.incoming,
                        bind: n['lambda'] as String?,
                        item: item,
                      ),
              ),
          ],
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

  void _tap(String? action) => web.window.parent?.postMessage(
        jsonEncode({'type': 'tap', 'action': action}).toJS,
        '*'.toJS,
      );


}

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
  static const sectionLabel = TextStyle(
    fontSize: 12,
    fontWeight: FontWeight.w700,
    letterSpacing: 0.6,
  );

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
        color: scheme.surfaceContainerHighest.withValues(alpha: 0.4),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(radiusCard)),
      ),
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

  factory Incoming.fromJson(Map<String, dynamic> j) => Incoming(
    screens: (j['screens'] as List?) ?? const [],
    text: (j['text'] as Map?)?.cast<String, dynamic>() ?? const {},
    locale: (j['locale'] as String?) ?? 'en',
    dark: (j['dark'] as bool?) ?? true,
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
                  Expanded(
                    child: Padding(
                      padding: const EdgeInsets.fromLTRB(kScreenPadH, Vaulet.sm, kScreenPadH, 0),
                      child: ListView(
                        children: [
                          for (final node in (screen['tree'] as List?) ?? const [])
                            _Node(node: node as Map<String, dynamic>, incoming: incoming),
                        ],
                      ),
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
  // Which tab is open is the host's. It is not application state, so it is not
  // hashed, not committed, and not a line in the execution record.
  int _tab = 0;

  Map<String, dynamic> get n => widget.node;
  List<Map<String, dynamic>> get children =>
      ((n['children'] as List?) ?? const []).cast<Map<String, dynamic>>();

  Map<String, dynamic> get args => ((n['args'] as Map?) ?? const {}).cast<String, dynamic>();

  /// The template is signed and the slots are not. The host fills and formats;
  /// an application that formatted a number would get Thai digits, the
  /// thousands separator and the currency position wrong separately from every
  /// other application.
  Widget _text({TextStyle? style}) {
    final key = args['text'] as String?;
    if (key == null) return const SizedBox.shrink();
    final entry = (widget.incoming.text[key] as Map?)?.cast<String, dynamic>();
    if (entry == null) {
      return Text(
        'missing key “$key”',
        style: TextStyle(color: Theme.of(context).colorScheme.error, fontSize: 12),
      );
    }
    final template = entry[widget.incoming.locale] as String?;
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
      final slot = args[m.group(1)] as String?;
      spans.add(
        WidgetSpan(
          alignment: PlaceholderAlignment.baseline,
          baseline: TextBaseline.alphabetic,
          child: Container(
            padding: const EdgeInsets.symmetric(horizontal: 3),
            decoration: BoxDecoration(
              color: slot == null
                  ? Theme.of(context).colorScheme.errorContainer
                  : Theme.of(context).colorScheme.surfaceContainerHighest,
              borderRadius: BorderRadius.circular(3),
            ),
            child: Text(
              slot ?? '${m.group(1)}?',
              style: const TextStyle(fontFamily: 'monospace', fontSize: 10),
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
                padding: const EdgeInsets.only(bottom: 8),
                child: _Node(node: c, incoming: widget.incoming),
              ),
          ],
        );

      case 'row':
        return Card(
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: Vaulet.md, vertical: 10),
            child: _text(),
          ),
        );

      case 'card':
        return Card(
          child: Padding(
            padding: const EdgeInsets.all(Vaulet.lg),
            child: _text(style: Vaulet.cardTitle),
          ),
        );

      case 'tabs':
        final tabs = children.where((c) => c['kind'] == 'tab').toList();
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            SegmentedButton<int>(
              showSelectedIcon: false,
              style: const ButtonStyle(visualDensity: VisualDensity.compact),
              segments: [
                for (var i = 0; i < tabs.length; i++)
                  ButtonSegment(
                    value: i,
                    label: Text(_label(tabs[i]), style: const TextStyle(fontSize: 11)),
                  ),
              ],
              selected: {_tab.clamp(0, tabs.isEmpty ? 0 : tabs.length - 1)},
              onSelectionChanged: (s) => setState(() => _tab = s.first),
            ),
            const SizedBox(height: 8),
            if (tabs.isNotEmpty)
              for (final c
                  in ((tabs[_tab.clamp(0, tabs.length - 1)]['children'] as List?) ?? const [])
                      .cast<Map<String, dynamic>>())
                Padding(
                  padding: const EdgeInsets.only(bottom: 6),
                  child: _Node(node: c, incoming: widget.incoming),
                ),
          ],
        );

      case 'list':
        final row = children.isEmpty ? null : children.first;
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            for (var i = 0; i < 3; i++)
              Opacity(
                opacity: i == 2 ? 0.4 : 1,
                child: Padding(
                  padding: const EdgeInsets.only(bottom: 6),
                  child: row == null
                      ? const Card(child: ListTile(dense: true, title: Text('row')))
                      : _Node(node: row, incoming: widget.incoming),
                ),
              ),
            Text(
              'the host draws the empty state too',
              style: Theme.of(context).textTheme.labelSmall,
            ),
          ],
        );

      case 'button':
        final primary = (args['emphasis'] as String?)?.trim() == 'primary';
        final action = (args['onTap'] as String?)?.trim();
        final child = _text(style: const TextStyle(fontSize: 13));
        // A press names an action. There is no other kind of handler, so
        // everything a screen can start goes through the same phases, the same
        // consent and the same record.
        void tap() => web.window.parent?.postMessage(
          jsonEncode({'type': 'tap', 'action': action}).toJS,
          '*'.toJS,
        );
        return Tooltip(
          message: action == null
              ? ''
              : 'calls $action, through require → verify → compute → update → execute',
          child: primary
              ? FilledButton(onPressed: tap, child: child)
              : OutlinedButton(onPressed: tap, child: child),
        );

      default:
        return const SizedBox.shrink();
    }
  }

  String _label(Map<String, dynamic> tab) {
    final raw = ((tab['args'] as Map?)?['text'] ?? (tab['args'] as Map?)?['0'] ?? '') as String;
    final entry = (widget.incoming.text[raw] as Map?)?.cast<String, dynamic>();
    return (entry?[widget.incoming.locale] as String?) ?? raw;
  }
}

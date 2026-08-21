// The playground's phone: a device around a screen the renderer draws.
//
// **Nothing here draws a VAL component.** That is `val_renderer`, which the
// wallet draws with too — two of it would agree for as long as somebody kept
// comparing them by eye. What is here is the illusion of a phone (the bezel,
// the status bar, the home indicator) and the plumbing that carries a screen in
// from the page and a press back out.

import 'dart:convert';
import 'dart:js_interop';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:val_renderer/val_renderer.dart';
import 'package:web/web.dart' as web;

void main() => runApp(const PreviewApp());


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

  /// A press the application declared, on its way back to the page. Moving
  /// between screens and coming back never gets here: those are the host's, and
  /// the renderer does them without asking.
  void _tap(String action, Map<String, Object?> input) {
    web.window.parent?.postMessage(
      jsonEncode({'type': 'tap', 'action': action, 'input': input}).toJS,
      '*'.toJS,
    );
  }

  /// A screen that takes parameters cannot have been resolved ahead of time —
  /// its content depends on the row that opened it — so the page is asked for
  /// it with what the press handed over.
  void _screen(String screen, Map<String, Object?> args) {
    web.window.parent?.postMessage(
      jsonEncode({'type': 'screen', 'screen': screen, 'args': args}).toJS,
      '*'.toJS,
    );
  }

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      // Flutter web leaves the mouse out of `dragDevices`, on the reasoning
      // that a page scrolls with the wheel. That is right for a page and wrong
      // for a phone on a desk: a carousel nobody can drag reads as a carousel
      // that does not work.
      scrollBehavior: const _DragAnywhere(),
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
                    child: _Phone(incoming: _in, onTap: _tap, onScreen: _screen),
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
  const _Phone({required this.incoming, required this.onTap, this.onScreen});

  final void Function(String, Map<String, Object?>) onTap;
  final void Function(String, Map<String, Object?>)? onScreen;

  static const width = 393.0;
  static const height = 852.0;

  /// The screen's corner, and the bezel around it. iPhone corners are a
  /// continuous curve rather than a circular one; `BorderRadius` is close
  /// enough at this size and the difference is not what anybody is here to
  /// check.
  static const screenRadius = 47.0;
  static const bezel = 12.0;

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
                    // The screen size the renderer asks about is this device's,
                    // not the browser window's: a carousel bleeding to "the
                    // edge of the screen" means this one.
                    child: MediaQuery(
                      data: MediaQuery.of(context).copyWith(size: const Size(width, height)),
                      child: ValApp(
                        incoming: incoming,
                        onTap: onTap,
                        onScreen: onScreen,
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


/// Drag with whatever is to hand — this is a phone being looked at through a
/// browser, and the pointer is standing in for a finger.
class _DragAnywhere extends MaterialScrollBehavior {
  const _DragAnywhere();

  @override
  Set<PointerDeviceKind> get dragDevices => const {
        PointerDeviceKind.touch,
        PointerDeviceKind.mouse,
        PointerDeviceKind.trackpad,
        PointerDeviceKind.stylus,
      };
}


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


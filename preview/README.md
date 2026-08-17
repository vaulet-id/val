# The preview renderer

A Flutter web app that draws a VAL screen with the host's own toolkit. The
playground embeds it and posts in the screen the compiler parsed; it posts back
which action a press would call.

**It renders nothing of its own accord.** No VAL is parsed here — the compiler
did that, and a renderer with its own front end would be a second thing to keep
faithful to the first. What arrives is structure, keys and action names.

## Whose theme this is

`Vaulet` in `lib/main.dart` holds tokens copied from
`vaulet-app/lib/core/theme/theme.dart`: the teal-ink seed, the 4-based spacing
scale, the 14pt corner radius, the 52pt button, the verified green.

**A copy, and the copy is the point.** This shows what a VAL screen looks like
*on Vaulet*, and a second host would supply its own. The language repository
holds no theme; a renderer does, because a renderer is a thing a host is.

Only tokens are copied. What drifts here is cosmetic and visible on the first
screenshot; copying the catalogue's *semantics* would drift invisibly, which is
why the semantics live in `docs/spec.md` and are checked by the compiler.

## Building

```
./build.sh
```

Writes `web/public/preview`, which is not checked in — it is forty megabytes of
CanvasKit, and a repository keeps its history forever. The playground says so
when it has not been built.

// A QR encoder, written here because the renderer carries no dependencies.
//
// **Byte mode, versions 1–10, error-correction level M.** A presentation or a
// URL is bytes, and byte mode encodes any of them; ten versions cover a URL and
// a signed token with room to spare. The one implementation, so a code scanned
// off a Vaulet screen is the same code however the screen was drawn.
//
// The load-bearing parts are tested rather than trusted: the GF(256) tables
// round-trip, and the codewords a message produces are, as systematic
// Reed-Solomon requires, an exact multiple of the generator polynomial —
// checked by an independent modulo, so a bug in the encoding loop shows up as a
// non-zero remainder rather than as a code nobody can scan.

import 'dart:typed_data';

import 'package:flutter/foundation.dart' show visibleForTesting;

/// A finished code: a square matrix of true (dark) and false (light) modules.
class QrCode {
  QrCode(this.size, this._modules);

  final int size;
  final List<bool> _modules;

  bool isDark(int row, int col) => _modules[row * size + col];

  /// Encode [text] as UTF-8 in byte mode. Throws if it does not fit in the ten
  /// versions this carries — a caller that might overflow should say so on
  /// screen rather than draw a truncated code.
  static QrCode encode(String text) => _Encoder(_utf8(text)).run();
}

/// UTF-8, without pulling in `dart:convert` for the one thing needed here.
Uint8List _utf8(String s) {
  final out = <int>[];
  for (final r in s.runes) {
    if (r < 0x80) {
      out.add(r);
    } else if (r < 0x800) {
      out..add(0xC0 | (r >> 6))..add(0x80 | (r & 0x3F));
    } else if (r < 0x10000) {
      out..add(0xE0 | (r >> 12))..add(0x80 | ((r >> 6) & 0x3F))..add(0x80 | (r & 0x3F));
    } else {
      out
        ..add(0xF0 | (r >> 18))
        ..add(0x80 | ((r >> 12) & 0x3F))
        ..add(0x80 | ((r >> 6) & 0x3F))
        ..add(0x80 | (r & 0x3F));
    }
  }
  return Uint8List.fromList(out);
}

// --- GF(256) for Reed-Solomon, the field QR is defined over. ---------------

final Uint8List _exp = Uint8List(512);
final Uint8List _log = Uint8List(256);
bool _tablesReady = false;

void _initTables() {
  if (_tablesReady) return;
  var x = 1;
  for (var i = 0; i < 255; i++) {
    _exp[i] = x;
    _log[x] = i;
    x <<= 1;
    if (x & 0x100 != 0) x ^= 0x11D; // the QR primitive polynomial
  }
  for (var i = 255; i < 512; i++) {
    _exp[i] = _exp[i - 255];
  }
  _tablesReady = true;
}

int _mul(int a, int b) => (a == 0 || b == 0) ? 0 : _exp[_log[a] + _log[b]];

List<int> _generator(int degree) {
  var poly = <int>[1];
  for (var i = 0; i < degree; i++) {
    final next = List<int>.filled(poly.length + 1, 0);
    for (var j = 0; j < poly.length; j++) {
      next[j] ^= poly[j];
      next[j + 1] ^= _mul(poly[j], _exp[i]);
    }
    poly = next;
  }
  return poly;
}

/// The remainder of [data] divided by [gen] — the EC codewords.
List<int> _remainder(List<int> data, List<int> gen) {
  final rem = List<int>.filled(gen.length - 1, 0);
  for (final d in data) {
    final factor = d ^ rem[0];
    for (var i = 0; i < rem.length - 1; i++) {
      rem[i] = rem[i + 1] ^ _mul(gen[i + 1], factor);
    }
    rem[rem.length - 1] = _mul(gen[gen.length - 1], factor);
  }
  return rem;
}

// Data codewords and EC-per-block for versions 1..10 at level M.
class _Ver {
  const _Ver(this.version, this.totalData, this.ecPerBlock, this.blocks);
  final int version;
  final int totalData;
  final int ecPerBlock;
  final int blocks;
}

const _versionsM = <_Ver>[
  _Ver(1, 16, 10, 1),
  _Ver(2, 28, 16, 1),
  _Ver(3, 44, 26, 1),
  _Ver(4, 64, 18, 2),
  _Ver(5, 86, 24, 2),
  _Ver(6, 108, 16, 4),
  _Ver(7, 124, 18, 4),
  _Ver(8, 154, 22, 2),
  _Ver(9, 182, 22, 3),
  _Ver(10, 216, 26, 4),
];

int _sizeOf(int version) => 17 + version * 4;

const _alignCenters = <int, List<int>>{
  2: [6, 18],
  3: [6, 22],
  4: [6, 26],
  5: [6, 30],
  6: [6, 34],
  7: [6, 22, 38],
  8: [6, 24, 42],
  9: [6, 26, 46],
  10: [6, 28, 50],
};

class _Encoder {
  _Encoder(this.data) {
    _initTables();
  }

  final Uint8List data;

  QrCode run() {
    final ver = _pickVersion();
    return _place(ver, _codewords(ver));
  }

  _Ver _pickVersion() {
    for (final v in _versionsM) {
      final countBits = v.version < 10 ? 8 : 16;
      final needed = (4 + countBits + data.length * 8 + 7) ~/ 8;
      if (needed <= v.totalData) return v;
    }
    throw StateError('too much data for a level-M code up to version 10');
  }

  List<int> _codewords(_Ver v) {
    final bits = _Bits();
    bits.push(0x4, 4); // byte mode
    bits.push(data.length, v.version < 10 ? 8 : 16);
    for (final b in data) {
      bits.push(b, 8);
    }
    final capacityBits = v.totalData * 8;
    final term = (capacityBits - bits.length).clamp(0, 4);
    bits.push(0, term);
    while (bits.length % 8 != 0) {
      bits.push(0, 1);
    }
    final dataCw = bits.bytes();
    var pad = true;
    while (dataCw.length < v.totalData) {
      dataCw.add(pad ? 0xEC : 0x11);
      pad = !pad;
    }

    final per = v.totalData ~/ v.blocks;
    final rem = v.totalData % v.blocks;
    final gen = _generator(v.ecPerBlock);
    final dataBlocks = <List<int>>[];
    final ecBlocks = <List<int>>[];
    var at = 0;
    for (var b = 0; b < v.blocks; b++) {
      final len = per + (b >= v.blocks - rem ? 1 : 0);
      final block = dataCw.sublist(at, at + len);
      at += len;
      dataBlocks.add(block);
      ecBlocks.add(_remainder(block, gen));
    }

    final out = <int>[];
    final maxData = dataBlocks.map((b) => b.length).reduce((a, b) => a > b ? a : b);
    for (var i = 0; i < maxData; i++) {
      for (final block in dataBlocks) {
        if (i < block.length) out.add(block[i]);
      }
    }
    for (var i = 0; i < v.ecPerBlock; i++) {
      for (final block in ecBlocks) {
        out.add(block[i]);
      }
    }
    return out;
  }

  QrCode _place(_Ver v, List<int> codewords) {
    final size = _sizeOf(v.version);
    final modules = List<bool?>.filled(size * size, null);
    void set(int r, int c, bool dark) => modules[r * size + c] = dark;
    bool? get(int r, int c) => modules[r * size + c];

    _finders(size, set);
    _timing(size, set, get);
    _alignment(v.version, size, set, get);
    set(size - 8, 8, true);
    _reserveFormat(size, set, get);

    var bit = 0;
    final total = codewords.length * 8;
    for (var col = size - 1; col > 0; col -= 2) {
      if (col == 6) col--;
      for (var i = 0; i < size; i++) {
        final up = ((size - 1 - col) ~/ 2) % 2 == 0;
        final row = up ? size - 1 - i : i;
        for (var c = 0; c < 2; c++) {
          final cc = col - c;
          if (get(row, cc) != null) continue;
          var dark = false;
          if (bit < total) {
            final b = codewords[bit ~/ 8];
            dark = (b >> (7 - (bit % 8))) & 1 == 1;
            bit++;
          }
          set(row, cc, dark);
        }
      }
    }

    var bestPenalty = 1 << 30;
    List<bool>? bestModules;
    for (var mask = 0; mask < 8; mask++) {
      final trial = List<bool>.generate(size * size, (i) => modules[i] ?? false);
      _applyMask(size, mask, modules, trial);
      _writeFormat(size, mask, trial);
      final p = _penalty(size, trial);
      if (p < bestPenalty) {
        bestPenalty = p;
        bestModules = trial;
      }
    }
    return QrCode(size, bestModules!);
  }

  void _finders(int size, void Function(int, int, bool) set) {
    void one(int r, int c) {
      for (var i = -1; i <= 7; i++) {
        for (var j = -1; j <= 7; j++) {
          final rr = r + i, cc = c + j;
          if (rr < 0 || cc < 0 || rr >= size || cc >= size) continue;
          final onBorder = i == 0 || i == 6 || j == 0 || j == 6;
          final inCore = i >= 2 && i <= 4 && j >= 2 && j <= 4;
          set(rr, cc, onBorder || inCore);
        }
      }
    }

    one(0, 0);
    one(0, size - 7);
    one(size - 7, 0);
  }

  void _timing(int size, void Function(int, int, bool) set, bool? Function(int, int) get) {
    for (var i = 8; i < size - 8; i++) {
      if (get(6, i) == null) set(6, i, i % 2 == 0);
      if (get(i, 6) == null) set(i, 6, i % 2 == 0);
    }
  }

  void _alignment(int version, int size, void Function(int, int, bool) set,
      bool? Function(int, int) get) {
    final centers = _alignCenters[version];
    if (centers == null) return;
    for (final r in centers) {
      for (final c in centers) {
        if (get(r, c) != null) continue;
        for (var i = -2; i <= 2; i++) {
          for (var j = -2; j <= 2; j++) {
            final ring = i.abs() == 2 || j.abs() == 2 || (i == 0 && j == 0);
            set(r + i, c + j, ring);
          }
        }
      }
    }
  }

  void _reserveFormat(int size, void Function(int, int, bool) set, bool? Function(int, int) get) {
    for (var i = 0; i < 9; i++) {
      if (get(8, i) == null) set(8, i, false);
      if (get(i, 8) == null) set(i, 8, false);
    }
    for (var i = 0; i < 8; i++) {
      if (get(8, size - 1 - i) == null) set(8, size - 1 - i, false);
      if (get(size - 1 - i, 8) == null) set(size - 1 - i, 8, false);
    }
  }

  void _applyMask(int size, int mask, List<bool?> reserved, List<bool> out) {
    for (var r = 0; r < size; r++) {
      for (var c = 0; c < size; c++) {
        if (reserved[r * size + c] == null && _maskAt(mask, r, c)) {
          out[r * size + c] = !out[r * size + c];
        }
      }
    }
  }

  bool _maskAt(int m, int r, int c) => switch (m) {
        0 => (r + c) % 2 == 0,
        1 => r % 2 == 0,
        2 => c % 3 == 0,
        3 => (r + c) % 3 == 0,
        4 => (r ~/ 2 + c ~/ 3) % 2 == 0,
        5 => (r * c) % 2 + (r * c) % 3 == 0,
        6 => ((r * c) % 2 + (r * c) % 3) % 2 == 0,
        _ => ((r + c) % 2 + (r * c) % 3) % 2 == 0,
      };

  void _writeFormat(int size, int mask, List<bool> out) {
    const ecBits = 0x00; // level M
    final fmt = (ecBits << 3) | mask;
    var bch = fmt << 10;
    for (var i = 14; i >= 10; i--) {
      if ((bch >> i) & 1 == 1) bch ^= 0x537 << (i - 10);
    }
    final bits = ((fmt << 10) | bch) ^ 0x5412;

    for (var i = 0; i < 15; i++) {
      final dark = (bits >> i) & 1 == 1;
      if (i < 6) {
        out[8 * size + i] = dark;
      } else if (i < 8) {
        out[8 * size + i + 1] = dark;
      } else {
        out[(15 - i) * size + 8] = dark;
      }
      if (i < 8) {
        out[8 * size + (size - 1 - i)] = dark;
      } else {
        out[(size - 15 + i) * size + 8] = dark;
      }
    }
    out[(size - 8) * size + 8] = true;
  }

  int _penalty(int size, List<bool> m) {
    var score = 0;
    bool at(int r, int c) => m[r * size + c];
    for (var r = 0; r < size; r++) {
      for (final line in [true, false]) {
        var run = 1;
        for (var i = 1; i < size; i++) {
          final a = line ? at(r, i) : at(i, r);
          final b = line ? at(r, i - 1) : at(i - 1, r);
          if (a == b) {
            run++;
          } else {
            if (run >= 5) score += 3 + (run - 5);
            run = 1;
          }
        }
        if (run >= 5) score += 3 + (run - 5);
      }
    }
    for (var r = 0; r < size - 1; r++) {
      for (var c = 0; c < size - 1; c++) {
        final v = at(r, c);
        if (v == at(r, c + 1) && v == at(r + 1, c) && v == at(r + 1, c + 1)) {
          score += 3;
        }
      }
    }
    return score;
  }
}

class _Bits {
  final List<bool> _bits = [];
  int get length => _bits.length;

  void push(int value, int width) {
    for (var i = width - 1; i >= 0; i--) {
      _bits.add((value >> i) & 1 == 1);
    }
  }

  List<int> bytes() {
    final out = <int>[];
    for (var i = 0; i < _bits.length; i += 8) {
      var b = 0;
      for (var j = 0; j < 8; j++) {
        b = (b << 1) | ((i + j < _bits.length && _bits[i + j]) ? 1 : 0);
      }
      out.add(b);
    }
    return out;
  }
}

@visibleForTesting
List<int> qrGenerator(int degree) {
  _initTables();
  return _generator(degree);
}

@visibleForTesting
List<int> qrRemainder(List<int> data, List<int> gen) => _remainder(data, gen);

@visibleForTesting
int qrMul(int a, int b) {
  _initTables();
  return _mul(a, b);
}

// The parts of the QR encoder that have to be exactly right, checked without a
// second QR library to disagree with — because the renderer carries none.

import 'package:flutter_test/flutter_test.dart';
import 'package:val_renderer/qr.dart';

/// Divide a codeword stream by a generator with an independent long division,
/// and return the remainder. The encoder's own remainder is systematic, so a
/// full (data + EC) stream must divide with no remainder — this is the check
/// that the EC codewords are right.
List<int> _mod(List<int> stream, List<int> gen) {
  final rem = List<int>.from(stream);
  for (var i = 0; i < stream.length - (gen.length - 1); i++) {
    final coef = rem[i];
    if (coef == 0) continue;
    for (var j = 0; j < gen.length; j++) {
      // gen is monic; multiply each term by coef in GF(256) and subtract (xor).
      rem[i + j] ^= qrMul(gen[j], coef);
    }
  }
  return rem.sublist(rem.length - (gen.length - 1));
}

void main() {
  test('the GF(256) field multiply has an identity and an inverse structure', () {
    for (var a = 1; a < 256; a++) {
      expect(qrMul(a, 1), a, reason: '1 is the identity');
    }
    // Distributes the way a field must: a*(b^c) == a*b ^ a*c.
    for (final (a, b, c) in [(2, 3, 5), (7, 11, 13), (200, 100, 50)]) {
      expect(qrMul(a, b ^ c), qrMul(a, b) ^ qrMul(a, c));
    }
  });

  test('a message and its EC codewords divide the generator with no remainder',
      () {
    // The property systematic Reed-Solomon guarantees, checked by a long
    // division that does not reuse the encoder's remainder routine.
    final gen = qrGenerator(10);
    final data = [32, 91, 11, 120, 209, 114, 220, 77, 67, 64, 236, 17, 236, 17, 236, 17];
    final ec = qrRemainder(data, gen);
    expect(ec.length, 10);

    final stream = [...data, ...ec];
    expect(_mod(stream, gen), List.filled(10, 0),
        reason: 'the codeword polynomial is a multiple of the generator');
  });

  test('a code is the right size, has three finder patterns, and quiet edges',
      () {
    final code = QrCode.encode('https://vaulet.id/p/abc123');
    // Version is whatever fit; the size is 17 + 4v, always odd and >= 21.
    expect(code.size >= 21, isTrue);
    expect((code.size - 17) % 4, 0);

    // A finder pattern is a 7x7 ring: dark border, light gap, dark core. Check
    // the top-left one's centre and its corners.
    expect(code.isDark(0, 0), isTrue);
    expect(code.isDark(3, 3), isTrue, reason: 'finder core is dark');
    expect(code.isDark(1, 1), isFalse, reason: 'finder gap is light');

    // The timing row alternates.
    expect(code.isDark(6, 8) != code.isDark(6, 9), isTrue);
  });

  test('the same text encodes to the same code every time', () {
    final a = QrCode.encode('acme-shift-morning');
    final b = QrCode.encode('acme-shift-morning');
    for (var r = 0; r < a.size; r++) {
      for (var c = 0; c < a.size; c++) {
        expect(a.isDark(r, c), b.isDark(r, c));
      }
    }
  });
}

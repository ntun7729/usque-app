import 'package:flutter_test/flutter_test.dart';
import 'package:usque/models/app_models.dart';

void main() {
  test('old network settings preserve the MASQUE default', () {
    final legacy = UsqueProfile.defaultProfile().toMap();
    legacy.remove('warp_underlay');
    legacy.remove('native_warp_profile');
    final loaded = UsqueProfile.fromMap(legacy);
    expect(loaded.warpUnderlay, WarpUnderlay.masque);
    expect(loaded.nativeWarpProfile, isNull);
    expect(loaded.toMap()['warp_underlay'], 'masque');
  });

  test('native WireGuard underlay selection round trips independently of chain', () {
    final initial = UsqueProfile.defaultProfile();
    final updated = initial.copyWith(warpUnderlay: WarpUnderlay.nativeWireGuard);
    final wire = updated.toMap();
    expect(wire['warp_underlay'], 'native_wireguard');
    expect(wire['chain_exit'], isNull);
    final restored = UsqueProfile.fromMap(wire);
    expect(restored.warpUnderlay, WarpUnderlay.nativeWireGuard);
    expect(restored.chainExit, isNull);
  });

  test('unknown outer WARP mode is rejected', () {
    final wire = UsqueProfile.defaultProfile().toMap();
    wire['warp_underlay'] = 'invalid';
    expect(() => UsqueProfile.fromMap(wire), throwsFormatException);
  });
}

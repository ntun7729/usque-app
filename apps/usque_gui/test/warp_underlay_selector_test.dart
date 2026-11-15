import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:usque/models/app_models.dart';
import 'package:usque/widgets/warp_underlay_selector.dart';

void main() {
  testWidgets('native WARP WireGuard is visible but disabled without runtime support',
      (tester) async {
    WarpUnderlay? requested;
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: WarpUnderlaySelector(
          selected: WarpUnderlay.masque,
          nativeAvailable: false,
          onSelected: (value) => requested = value,
        ),
      ),
    ));

    expect(find.text('Native WireGuard UDP'), findsOneWidget);
    expect(find.textContaining('not available'), findsOneWidget);
    final selector = tester.widget<SegmentedButton<WarpUnderlay>>(
      find.byType(SegmentedButton<WarpUnderlay>),
    );
    expect(selector.selected, {WarpUnderlay.masque});
    expect(
      selector.segments
          .singleWhere((segment) => segment.value == WarpUnderlay.nativeWireGuard)
          .enabled,
      isFalse,
    );
    expect(requested, isNull);
  });

  testWidgets('available native WARP can be selected without altering a chain profile',
      (tester) async {
    WarpUnderlay? requested;
    const chain = ChainExitSettings(
      enabled: true,
      source: ChainSource.socks5Proxy,
      profileId: 'preset',
      revision: 'revision',
    );
    final profile = UsqueProfile.defaultProfile().copyWith(chainExit: chain);

    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: WarpUnderlaySelector(
          selected: profile.warpUnderlay,
          nativeAvailable: true,
          onSelected: (value) => requested = value,
        ),
      ),
    ));

    await tester.tap(find.text('Native WireGuard UDP'));
    await tester.pump();
    expect(requested, WarpUnderlay.nativeWireGuard);
    final next = profile.copyWith(warpUnderlay: requested);
    expect(next.chainExit, chain);
  });
}

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:usque/core/usque_theme.dart';
import 'package:usque/models/app_models.dart';
import 'package:usque/screens/advanced_settings_screen.dart';
import 'package:usque/services/engine_client.dart';
import 'package:usque/state/app_controller.dart';
import 'package:usque/widgets/warp_underlay_selector.dart';

import 'app_test.dart' show FakeEngineClient;

class _OuterWarpLibraryEngine extends FakeEngineClient
    implements ChainProfileClient {
  @override
  Future<EngineCapabilities?> getCapabilities() async =>
      const EngineCapabilities(
        networkSettingsApplication: true,
        nativeWarpWireguard: true,
        tlsFingerprints: TlsFingerprint.values,
        automaticEndpoints: true,
      );

  @override
  Future<ChainProfileResult> chainProfile(Map<String, Object?> request) async =>
      const ChainProfileResult(profiles: [
        ChainProfileSummary(
          id: 'outer-warp',
          revision: 'outer-rev',
          editRevision: 'outer-edit',
          name: 'WARP outer UDP',
          protocol: 'wireguard',
          host: '162.159.192.17',
          port: 500,
          source: ChainSource.warpWireguard,
        ),
      ]);

  @override
  Future<List<ChainConfigurationFile>> pickChainConfigurations() async => [];
}

void main() {
  testWidgets('advanced settings reveals native WARP profile picker when supported',
      (tester) async {
    tester.view.physicalSize = const Size(1000, 2300);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    SharedPreferences.setMockInitialValues({'update_checks_enabled': false});
    final controller = AppController(_OuterWarpLibraryEngine());
    await controller.initialize();
    addTearDown(controller.dispose);
    await tester.pumpWidget(MaterialApp(
      theme: UsqueTheme.light(),
      home: AdvancedSettingsScreen(controller: controller),
    ));
    await tester.pump();
    await tester.ensureVisible(find.text('Native WireGuard UDP'));
    await tester.tap(find.text('Native WireGuard UDP'));
    await tester.pump();
    expect(find.byType(NativeWarpProfilePicker), findsOneWidget);
  });

  testWidgets('advanced settings exposes the WARP carrier selector', (tester) async {
    SharedPreferences.setMockInitialValues({'update_checks_enabled': false});
    final controller = AppController(FakeEngineClient());
    await controller.initialize();
    addTearDown(controller.dispose);
    await tester.pumpWidget(MaterialApp(
      theme: UsqueTheme.light(),
      home: AdvancedSettingsScreen(controller: controller),
    ));
    await tester.pump();

    expect(find.byType(WarpUnderlaySelector), findsOneWidget);
    expect(find.text('Native WireGuard UDP'), findsOneWidget);
  });

  testWidgets('native WARP profile picker filters out final chain proxy sources',
      (tester) async {
    final profiles = [
      const ChainProfileSummary(
        id: 'warp-id',
        revision: 'warp-revision',
        editRevision: 'warp-edit',
        name: 'WARP UDP 500',
        protocol: 'wireguard',
        host: '162.159.192.17',
        port: 500,
        source: ChainSource.warpWireguard,
      ),
      const ChainProfileSummary(
        id: 'proxy-id',
        revision: 'proxy-revision',
        editRevision: 'proxy-edit',
        name: 'SOCKS5 final exit',
        protocol: 'socks5',
        host: '127.0.0.1',
        port: 1080,
        source: ChainSource.socks5Proxy,
      ),
    ];
    ChainExitSettings? selected;
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: NativeWarpProfilePicker(
          profiles: profiles,
          selected: null,
          onSelected: (value) => selected = value,
        ),
      ),
    ));
    await tester.tap(find.byKey(const ValueKey('native-warp-profile-picker')));
    await tester.pumpAndSettle();
    expect(find.text('WARP UDP 500').last, findsOneWidget);
    expect(find.text('SOCKS5 final exit'), findsNothing);
    await tester.tap(find.text('WARP UDP 500').last);
    await tester.pumpAndSettle();
    expect(selected?.profileId, 'warp-id');
    expect(selected?.revision, 'warp-revision');
    expect(selected?.source, ChainSource.warpWireguard);
    expect(selected?.enabled, isTrue);
  });

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

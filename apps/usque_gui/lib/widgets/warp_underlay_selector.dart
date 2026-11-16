import 'package:flutter/material.dart';

import '../models/app_models.dart';

/// The physical WARP carrier is independent of whichever chain exit is used.
///
/// Only enable native WireGuard when the engine advertises an actual working
/// underlay. Compiling a WireGuard packet actor is not sufficient capability.
class WarpUnderlaySelector extends StatelessWidget {
  const WarpUnderlaySelector({
    required this.selected,
    required this.nativeAvailable,
    required this.onSelected,
    this.title = 'WARP connection',
    this.masqueLabel = 'MASQUE H3/H2',
    this.nativeLabel = 'Native WireGuard UDP',
    this.unavailableText =
        'Native WireGuard UDP is not available in this build. MASQUE remains active.',
    super.key,
  });

  final WarpUnderlay selected;
  final bool nativeAvailable;
  final ValueChanged<WarpUnderlay>? onSelected;
  final String title;
  final String masqueLabel;
  final String nativeLabel;
  final String unavailableText;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: <Widget>[
      Text(title, style: Theme.of(context).textTheme.titleSmall),
      const SizedBox(height: 10),
      LayoutBuilder(
        builder: (context, constraints) => SegmentedButton<WarpUnderlay>(
          direction:
              constraints.maxWidth < 560 ||
                  MediaQuery.textScalerOf(context).scale(1) > 1.3
              ? Axis.vertical
              : Axis.horizontal,
          segments: <ButtonSegment<WarpUnderlay>>[
            ButtonSegment<WarpUnderlay>(
              value: WarpUnderlay.masque,
              label: Text(masqueLabel),
            ),
            ButtonSegment<WarpUnderlay>(
              value: WarpUnderlay.nativeWireGuard,
              label: Text(nativeLabel),
              enabled: nativeAvailable,
              tooltip: nativeAvailable ? null : unavailableText,
            ),
          ],
          selected: <WarpUnderlay>{selected},
          onSelectionChanged: onSelected == null
              ? null
              : (selection) {
                  final mode = selection.single;
                  if (!nativeAvailable &&
                      mode == WarpUnderlay.nativeWireGuard) {
                    return;
                  }
                  onSelected!(mode);
                },
          showSelectedIcon: false,
        ),
      ),
      if (!nativeAvailable) ...<Widget>[
        const SizedBox(height: 8),
        Text(
          unavailableText,
          style: Theme.of(context).textTheme.bodySmall,
        ),
      ],
    ],
  );
}


/// Reuses an existing encrypted WARP WireGuard library profile as the outer
/// carrier. Other imported chain profiles must never appear in this picker.
class NativeWarpProfilePicker extends StatelessWidget {
  const NativeWarpProfilePicker({
    required this.profiles,
    required this.selected,
    required this.onSelected,
    this.label = 'WARP WireGuard configuration',
    this.emptyMessage = 'Import or generate a WARP via WireGuard profile in Chain Proxy.',
    super.key,
  });

  final List<ChainProfileSummary> profiles;
  final ChainExitSettings? selected;
  final ValueChanged<ChainExitSettings?>? onSelected;
  final String label;
  final String emptyMessage;

  @override
  Widget build(BuildContext context) {
    final available = profiles
        .where((profile) => profile.source == ChainSource.warpWireguard)
        .toList(growable: false);
    final chosen = available
        .where((item) =>
            item.id == selected?.profileId &&
            item.revision == selected?.revision)
        .firstOrNull;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        DropdownButtonFormField<String>(
          key: const ValueKey('native-warp-profile-picker'),
          initialValue: chosen?.id,
          isExpanded: true,
          decoration: InputDecoration(labelText: label),
          hint: const Text('Choose a saved WARP configuration'),
          items: available.map((profile) => DropdownMenuItem<String>(
            value: profile.id,
            child: Text(profile.name, overflow: TextOverflow.ellipsis),
          )).toList(growable: false),
          onChanged: available.isEmpty || onSelected == null
              ? null
              : (id) {
                  final selection = available.where((p) => p.id == id).firstOrNull;
                  if (selection == null) return;
                  final same = selected?.profileId == selection.id &&
                      selected?.revision == selection.revision;
                  onSelected!(same
                      ? selected
                      : ChainExitSettings(
                          enabled: true,
                          source: ChainSource.warpWireguard,
                          profileId: selection.id,
                          revision: selection.revision,
                        ));
                },
        ),
        if (available.isEmpty) ...<Widget>[
          const SizedBox(height: 8),
          Text(emptyMessage, style: Theme.of(context).textTheme.bodySmall),
        ],
        if (chosen != null) ...<Widget>[
          const SizedBox(height: 4),
          Text(
            '${chosen.host}:${chosen.port} · ${chosen.addressFamily}',
            style: Theme.of(context).textTheme.bodySmall,
          ),
        ],
      ],
    );
  }
}

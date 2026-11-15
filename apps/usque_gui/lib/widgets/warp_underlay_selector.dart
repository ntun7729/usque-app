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

import 'package:flutter/material.dart';

import '../theme/theme.dart';

/// Inline "App" badge shown beside an App-attributed message author.
class AppBadge extends StatelessWidget {
  /// Creates the named App attribution badge.
  const AppBadge({super.key});

  @override
  Widget build(BuildContext context) {
    return Container(
      key: const ValueKey('message-app-badge'),
      padding: const EdgeInsets.symmetric(
        horizontal: Grid.half,
        vertical: Grid.quarter,
      ),
      decoration: BoxDecoration(
        color: context.colors.secondaryContainer,
        borderRadius: BorderRadius.circular(Radii.sm),
      ),
      child: Text(
        'App',
        style: context.textTheme.labelSmall?.copyWith(
          color: context.colors.onSecondaryContainer,
          fontWeight: FontWeight.w600,
        ),
      ),
    );
  }
}

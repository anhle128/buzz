part of '../thread_detail_page.dart';

class _Avatar extends StatelessWidget {
  final UserProfile? profile;
  final String pubkey;
  final String? imageUrl;
  final String? fallbackLabel;

  const _Avatar({
    required this.profile,
    required this.pubkey,
    this.imageUrl,
    this.fallbackLabel,
  });

  @override
  Widget build(BuildContext context) {
    final label = fallbackLabel?.trim();
    final initial = label != null && label.isNotEmpty
        ? label[0].toUpperCase()
        : profile?.initial ??
              (pubkey.isNotEmpty ? pubkey[0].toUpperCase() : '?');
    final avatarUrl = imageUrl ?? profile?.avatarUrl;

    return AvatarImage(
      imageUrl: avatarUrl,
      radius: messageAvatarSize / 2,
      backgroundColor: context.colors.primaryContainer,
      fallback: Text(
        initial,
        style: context.textTheme.labelMedium?.copyWith(
          color: context.colors.onPrimaryContainer,
          fontWeight: FontWeight.w600,
        ),
      ),
    );
  }
}

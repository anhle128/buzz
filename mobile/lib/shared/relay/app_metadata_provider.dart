import 'package:hooks_riverpod/hooks_riverpod.dart';

import '../community/relay_information_provider.dart';
import 'app_metadata.dart';
import 'nostr_models.dart';
import 'relay_provider.dart';
import 'relay_session.dart';

/// Community-scoped map of verified App metadata keyed by canonical App UUID.
final appMetadataProvider =
    FutureProvider.autoDispose<Map<String, AppMetadata>>((ref) async {
      ref.watch(relayConfigProvider);
      final sessionState = ref.watch(relaySessionProvider);
      final self = await ref.watch(relaySelfProvider.future);
      if (self == null || sessionState.status != SessionStatus.connected) {
        return const {};
      }

      try {
        final events = await ref
            .read(relaySessionProvider.notifier)
            .fetchHistory(
              NostrFilter(
                kinds: const [EventKind.appMetadata],
                authors: [self],
                limit: 500,
              ),
            );
        return foldAppMetadataHeads(events, self);
      } catch (_) {
        return const {};
      }
    });

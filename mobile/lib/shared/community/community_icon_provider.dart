import 'package:hooks_riverpod/hooks_riverpod.dart';

import 'relay_information_provider.dart';

/// Supplies the HTTP client used for NIP-11 community icon lookups.
///
/// Tests can override this provider to return deterministic relay responses.
final communityIconHttpClientProvider = relayInformationHttpClientProvider;

/// Reads a community's icon from its public NIP-11 relay information document.
///
/// The lookup does not depend on the active relay session, so icons can render
/// for every paired community in the switcher. Callers explicitly invalidate
/// this family when opening the switcher so transient failures and relay
/// metadata updates can be retried without background polling.
final communityIconProvider = FutureProvider.autoDispose
    .family<String?, String>((ref, relayUrl) async {
      return (await loadRelayInformation(
        ref.read(communityIconHttpClientProvider),
        relayUrl,
      )).icon;
    });

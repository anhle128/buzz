import 'package:buzz/shared/community/relay_information_provider.dart';
import 'package:buzz/shared/relay/relay_provider.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart' as http_testing;

const _validSelf =
    'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
const _otherSelf =
    'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';

void main() {
  test('reads a valid NIP-11 self pubkey over the relay HTTP URL', () async {
    late http.Request capturedRequest;
    final client = http_testing.MockClient((request) async {
      capturedRequest = request;
      return http.Response('{"self":"${_validSelf.toUpperCase()}"}', 200);
    });
    final container = ProviderContainer(
      overrides: [relayInformationHttpClientProvider.overrideWithValue(client)],
    );
    addTearDown(container.dispose);

    final info = await container.read(
      relayInformationProvider('wss://relay.example.com').future,
    );

    expect(capturedRequest.url, Uri.parse('https://relay.example.com'));
    expect(capturedRequest.headers['Accept'], 'application/nostr+json');
    expect(info.self, _validSelf);
  });

  test('returns null self when the NIP-11 document omits self', () async {
    final client = http_testing.MockClient(
      (_) async =>
          http.Response('{"icon":"https://relay.example.com/i.png"}', 200),
    );
    final container = ProviderContainer(
      overrides: [relayInformationHttpClientProvider.overrideWithValue(client)],
    );
    addTearDown(container.dispose);

    final info = await container.read(
      relayInformationProvider('https://relay.example.com').future,
    );

    expect(info.self, isNull);
    expect(info.icon, 'https://relay.example.com/i.png');
  });

  test('returns null self for malformed NIP-11 self values', () async {
    Future<String?> selfFor(String body) async {
      final client = http_testing.MockClient(
        (_) async => http.Response(body, 200),
      );
      final container = ProviderContainer(
        overrides: [
          relayInformationHttpClientProvider.overrideWithValue(client),
        ],
      );
      addTearDown(container.dispose);
      return (await container.read(
        relayInformationProvider('https://relay.example.com').future,
      )).self;
    }

    expect(await selfFor('{"self":""}'), isNull);
    expect(await selfFor('{"self":"not-a-key"}'), isNull);
    expect(await selfFor('{"self":"abcd"}'), isNull);
    expect(await selfFor('{"self":123}'), isNull);
  });

  test(
    'returns empty information when relay information cannot be loaded',
    () async {
      final client = http_testing.MockClient(
        (_) async => http.Response('unavailable', 503),
      );
      final container = ProviderContainer(
        overrides: [
          relayInformationHttpClientProvider.overrideWithValue(client),
        ],
      );
      addTearDown(container.dispose);

      final info = await container.read(
        relayInformationProvider('https://relay.example.com').future,
      );

      expect(info.self, isNull);
      expect(info.icon, isNull);
    },
  );

  test('re-reads self after the active community relay URL changes', () async {
    final client = http_testing.MockClient((request) async {
      if (request.url.host == 'one.example.com') {
        return http.Response('{"self":"$_validSelf"}', 200);
      }
      if (request.url.host == 'two.example.com') {
        return http.Response('{"self":"$_otherSelf"}', 200);
      }
      return http.Response('missing', 404);
    });
    final container = ProviderContainer(
      overrides: [
        relayInformationHttpClientProvider.overrideWithValue(client),
        relayConfigProvider.overrideWith(_MutableRelayConfig.new),
      ],
    );
    addTearDown(container.dispose);
    final subscription = container.listen(relaySelfProvider, (_, _) {});
    addTearDown(subscription.close);

    expect(await container.read(relaySelfProvider.future), _validSelf);

    container
        .read(relayConfigProvider.notifier)
        .update(baseUrl: 'https://two.example.com');

    expect(await container.read(relaySelfProvider.future), _otherSelf);
  });
}

class _MutableRelayConfig extends RelayConfigNotifier {
  @override
  RelayConfig build() => const RelayConfig(baseUrl: 'https://one.example.com');
}

import assert from "node:assert/strict";
import test from "node:test";

import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterContextProvider,
} from "@tanstack/react-router";

import { ThreadPreviewRow } from "./ChannelActivityPopover.tsx";

const ssrRouter = createRouter({
  history: createMemoryHistory({ initialEntries: ["/"] }),
  routeTree: createRootRoute({}),
});

test("thread preview identifies App-authored activity", () => {
  const html = renderToStaticMarkup(
    React.createElement(
      RouterContextProvider,
      { router: ssrRouter },
      React.createElement(ThreadPreviewRow, {
        item: {
          avatarUrl: null,
          conversationId: "root-event",
          id: "reply-event",
          isApp: true,
          mentionNames: [],
          preview: "Build passed",
          senderLabel: "Buildkite",
          timestampLabel: "now",
          unreadCount: 1,
        },
        onMarkRead() {},
        onOpen() {},
        onRemindLater() {},
      }),
    ),
  );

  assert.match(html, /data-testid="message-app-badge"/);
  assert.match(html, />App</);
});

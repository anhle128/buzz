import assert from "node:assert/strict";
import test from "node:test";

class ElementShim {
  constructor() {
    this.children = [];
    this.childNodes = [];
    this.nodeType = 1;
    this.nodeName = "DIV";
    this.tagName = "DIV";
    this.namespaceURI = "http://www.w3.org/1999/xhtml";
  }
  get ownerDocument() {
    return globalThis.document;
  }
  addEventListener() {}
  removeEventListener() {}
  appendChild(child) {
    this.children.push(child);
    this.childNodes.push(child);
    return child;
  }
  removeChild(child) {
    this.children = this.children.filter((current) => current !== child);
    this.childNodes = this.childNodes.filter((current) => current !== child);
    return child;
  }
  insertBefore(child) {
    return this.appendChild(child);
  }
  contains(target) {
    return this === target;
  }
}

globalThis.document = {
  activeElement: null,
  addEventListener() {},
  createElement: () => new ElementShim(),
  get defaultView() {
    return globalThis.window;
  },
  nodeType: 9,
  removeEventListener() {},
};
Object.defineProperty(globalThis, "window", {
  configurable: true,
  value: {
    addEventListener() {},
    document: globalThis.document,
    event: undefined,
    HTMLIFrameElement: ElementShim,
    removeEventListener() {},
  },
});
globalThis.HTMLElement = ElementShim;
globalThis.Node = ElementShim;
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

import React, { act } from "react";
import { createRoot } from "react-dom/client";

import { usePermissionAlertToastTracker } from "./usePendingPermissionAlerts.ts";

test("permission toast tracker dismisses unresolved toasts on unmount without redismissing resolved ones", async () => {
  const dismissed = [];
  const control = {};

  function Harness() {
    control.current = usePermissionAlertToastTracker((nonce) =>
      dismissed.push(nonce),
    );
    return null;
  }

  const root = createRoot(new ElementShim());
  await act(async () => root.render(React.createElement(Harness)));
  await act(async () => {
    control.current.track("pending-nonce");
    control.current.track("resolved-nonce");
    control.current.dismiss("resolved-nonce");
  });

  assert.deepEqual(dismissed, ["resolved-nonce"]);

  await act(async () => root.unmount());
  assert.deepEqual(dismissed, ["resolved-nonce", "pending-nonce"]);
});

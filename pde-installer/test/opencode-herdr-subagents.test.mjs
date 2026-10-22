import assert from "node:assert/strict";
import test from "node:test";
import { keepWorkingForSubagents } from "../../chezmoi/dot_config/opencode/pde-herdr-opencode/subagents.js";

function fakeClock() {
  let time = 0;
  const timers = new Map();
  return {
    now: () => time,
    setInterval: (fn, ms) => {
      timers.set(fn, ms);
      return fn;
    },
    clearInterval: (fn) => timers.delete(fn),
    advance(ms) {
      const step = Math.min(...timers.values());
      for (let passed = 0; passed < ms; passed += step) {
        time += step;
        for (const fn of timers.keys()) fn();
      }
    },
  };
}

function openCodeWithSubagent(clock) {
  const info = new Map([["root", { id: "root" }], ["child", { id: "child", parentID: "root" }]]);
  const statuses = new Map([["root", "running"], ["child", "running"]]);
  let deliver;
  const api = {
    data: {
      session: {
        get: (id) => info.get(id),
        family: () => [...info.keys()],
        status: (id) => statuses.get(id) ?? "idle",
      },
      listen(callback) {
        deliver = callback;
        return () => {};
      },
    },
  };
  return {
    api: keepWorkingForSubagents(api, clock),
    statuses,
    emit: (type, sessionID) => deliver({ details: { type, data: { sessionID } } }),
  };
}

function listenForEvents(api) {
  const events = [];
  const stop = api.data.listen(({ details }) => events.push(`${details.type} ${details.data.sessionID}`));
  return { events, stop };
}

test("holds root turn end until subagents stop", () => {
  const clock = fakeClock();
  const openCode = openCodeWithSubagent(clock);
  const herdr = listenForEvents(openCode.api);

  openCode.statuses.set("root", "idle");
  openCode.emit("session.execution.succeeded", "root");
  openCode.emit("permission.asked", "child");
  const whileSubagentRuns = [...herdr.events];
  openCode.statuses.set("child", "idle");
  openCode.emit("session.execution.succeeded", "child");
  clock.advance(1000);
  herdr.stop();

  assert.deepEqual(whileSubagentRuns, ["permission.asked child"]);
  assert.deepEqual(herdr.events, [
    "permission.asked child",
    "session.execution.succeeded child",
    "session.execution.succeeded root",
  ]);
});

test("wake-up turn drops the held end", () => {
  const clock = fakeClock();
  const openCode = openCodeWithSubagent(clock);
  const herdr = listenForEvents(openCode.api);

  openCode.statuses.set("root", "idle");
  openCode.emit("session.execution.succeeded", "root");
  openCode.statuses.set("child", "idle");
  openCode.emit("session.execution.succeeded", "child");
  clock.advance(250);
  openCode.statuses.set("root", "running");
  openCode.emit("session.execution.started", "root");
  clock.advance(2000);
  herdr.stop();

  assert.deepEqual(herdr.events, ["session.execution.succeeded child", "session.execution.started root"]);
});

test("root reads running while subagents run", () => {
  const openCode = openCodeWithSubagent(fakeClock());
  openCode.statuses.set("root", "idle");

  const whileSubagentRuns = openCode.api.data.session.status("root");
  openCode.statuses.set("child", "idle");
  const afterSubagentStops = openCode.api.data.session.status("root");

  assert.equal(whileSubagentRuns, "running");
  assert.equal(afterSubagentStops, "idle");
});

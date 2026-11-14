# Set Up Approval

Do this once on each computer where agents write to your pull requests,
so their requests reach you in Herdr and on your phone. Keep your normal
`gh` login; approval needs no token of its own.

1. Install the full profile:

   ```bash
   pde-installer install full
   ```

   [How it gets installed](../explanation/request-flow.md#how-it-gets-installed)
   says what this puts in place.

2. Restart every OpenCode server and client. A server started before the
   install keeps its old `PATH`, so its agents can get past the guard; see
   [OpenCode server refresh](../../../pde-installer/docs/explanation/opencode-server-refresh.md).
3. Check that the Herdr plugin is linked, as in
   [Debug a stuck or unanswered request](debug-a-stuck-request.md#5-if-no-popup-appeared).
4. Pair your phone and start the `moshi-hook` background service, which
   sends the phone question. `moshi-hook pair --help` says the token is
   in the Moshi iPhone app under Settings, then Integrations.

   ```bash
   moshi-hook pair --token TOKEN
   moshi-hook service install
   ```

5. Check that `moshi-hook status` prints `status: paired` and
   `moshi-hook service status` shows `active (running)`. To test, run
   `moshi-hook ask --require-remote --timeout 2m "Test"` and answer on
   the phone; it exits 0 or 1 when the phone works.
6. Optional: ask an agent to post a test comment on one of your pull
   requests, then approve it as in
   [Approve or decline a request](approve-a-request.md).

# 0022. UI tooling: Vite, Biome, TypeScript 7 and Vitest in the browser, on npm

Status: proposed. Date: 2026-11-07. Follows [0012](0012-ui-shell.md), which leaves build tooling to the first slice.

## Choice

| Job | Pick | Version checked | Licence |
|---|---|---|---|
| Runtime and packages | Node 24 LTS and npm, with `package-lock.json` | Node 24.19, npm 11.17 | npm: Artistic-2.0 |
| Bundler and dev server | Vite with `@vitejs/plugin-react` | 8.3.6, 6.2.1 | MIT |
| Linter and formatter | Biome, one tool for both | 2.6.6 | MIT OR Apache-2.0 |
| Type check | TypeScript, `tsc --noEmit` | 7.0.2 | Apache-2.0 |
| Test runner | Vitest in browser mode, Playwright provider, `vitest-browser-react` | 5.1.0, Playwright 1.65.0, 2.3.0 | MIT, Apache-2.0, MIT |

Versions are the npm `latest` on the date above. The lockfile pins them.

- **Where:** the UI is its own npm package in `ui/`. Nothing in the repo root changes for people working only on the service.
- **Vite** uses the settings in Tauri's [Vite guide](https://v2.tauri.app/start/frontend/vite/): `clearScreen: false`, a fixed dev port with `strictPort: true`, and `envPrefix: ['VITE_', 'TAURI_ENV_*']`. The build target stays Vite's default until the oldest supported macOS is decided (see "Checked, and not yet").
- **Biome** lints and formats TypeScript, TSX and CSS with its recommended rules, which include the React hook rules. No ESLint, no Prettier.
- **Tests** run in a real browser, driven by real input: Chromium on Windows (the WebView2 engine) and WebKit on macOS (the WKWebView engine). They sit at the "Shared UI components" seam in [testing.md](../testing.md). They click and type, then read the page back, never trusting that a click acted (ledger S31b). `retry` stays at its default of 0: a flaky test is fixed or deleted (testing.md).
- **Reporters** stay at Vitest's defaults, so CI keeps the `github-actions` reporter, which is added only when the list isn't overridden.

### CI

A `UI` job joins `.github/workflows/ci.yml`, on Windows and macOS like the Rust job, run in `ui/`:

1. `actions/setup-node@v7` with `node-version-file: ui/.node-version`, `cache: npm` and `cache-dependency-path: ui/package-lock.json` (it looks in the repo root otherwise, and `working-directory` only applies to `run` steps)
2. `npm ci`
3. `npx playwright install chromium` on Windows, `webkit` on macOS
4. `npx biome ci`
5. `npx tsc --noEmit`
6. `npx vitest run`
7. `npx vite build`

`scripts/check-file-size.sh` already covers `.ts`, `.tsx`, `.css` and `.html`. The job and the AGENTS.md "Build and check" lines land in part C's first PR, with the first UI code. Until then there's nothing for them to check.

## Why

- **Vite is what Tauri recommends:** "For most projects we recommend Vite for SPA frameworks such as React" ([Tauri frontend](https://v2.tauri.app/start/frontend/)). Vite 8 bundles with Rolldown and compiles with Oxc, both in Rust ([Vite 8](https://vite.dev/blog/announcing-vite8)). It needs Node 20.19+ or 22.12+ ([guide](https://vite.dev/guide/)). The previous app also used Vite (7), so its lessons carry over.
- **A failing test names itself (ledger S14, S66).** Vitest's default reporter prints a `Failed Tests N` section with one ` FAIL ` line per test: its full name, then its file, then its error and diff ([`base.ts`, `printErrorsSummary` and `printTaskErrors`](https://github.com/vitest-dev/vitest/blob/f21f224bad11e738840bda1f4b6d64dd7de4a425/packages/vitest/src/node/reporters/base.ts#L1284-L1545)). Failed suites and unhandled errors get sections of their own. An unhandled error fails the run unless `dangerouslyIgnoreUnhandledErrors` is set, and its default is `false` ([config](https://vitest.dev/config/dangerouslyignoreunhandlederrors)). The previous app's `node --test` reported `fail 1` without naming the test.
- **Agents get a short report.** When Vitest detects an AI coding agent it picks the `minimal` reporter, which "outputs a minimal report containing only failed tests and their error messages" ([reporters](https://vitest.dev/guide/reporters)). That's the lean context [0020](0020-context-per-turn.md) asks for. In GitHub Actions, failures also show as annotations on the PR (same page).
- **Real input, real engines.** Browser mode is stable as of Vitest 4 ([Vitest 4](https://vitest.dev/blog/vitest-4)). With the Playwright provider, `userEvent` acts "using Chrome DevTools Protocol or webdriver instead of faking events" ([interactivity API](https://vitest.dev/guide/browser/interactivity-api)). The `preview` provider simulates events, so it isn't used. Playwright drives Chromium and WebKit on Windows and macOS ([intro](https://playwright.dev/docs/intro)). It says "for the closest-to-Safari experience you should run WebKit on mac" ([browsers](https://playwright.dev/docs/browsers)), hence WebKit on the macOS runner. Vitest 5 needs Vite 6.4 or later and Node 22.12 or later ([guide](https://vitest.dev/guide/)). `vitest-browser-react` "render[s] React components in Vitest Browser Mode" and accepts Vitest 4 or 5 ([repo](https://github.com/vitest-community/vitest-browser-react)).
- **Biome is one tool, with no TypeScript compiler.** `biome ci` runs every check, has no write option, and prints GitHub annotations when run on GitHub ([CI recipe](https://biomejs.dev/recipes/continuous-integration/)). `useExhaustiveDependencies` and `useHookAtTopLevel` are on by default ([rules](https://biomejs.dev/linter/rules/use-exhaustive-dependencies/)). Biome formats and lints CSS too ([language support](https://biomejs.dev/internals/language-support/)). Its type-aware rules don't need the `typescript` package ([Biome v2](https://biomejs.dev/blog/biome-v2/)), which matters because of the next point.
- **TypeScript 7 is the native compiler and only type-checks here.** It is "a 10x faster native port", and "does not ship with an API" until 7.1 ([TypeScript 7.0](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/)). Vite and Biome don't need that API, so nothing is lost. `noEmit` and `jsx: react-jsx` are both in its option tables ([`declscompiler.go`](https://github.com/microsoft/typescript-go/blob/89d5d5b2849a0db0957065889ca58536fa6d2e4a/internal/tsoptions/declscompiler.go#L197), [`enummaps.go`](https://github.com/microsoft/typescript-go/blob/89d5d5b2849a0db0957065889ca58536fa6d2e4a/internal/tsoptions/enummaps.go#L197)).
- **Node 24** is on this PC and is LTS until 2028-04-30 ([schedule](https://github.com/nodejs/release#release-schedule)). Every pick above supports it. **npm** comes with Node, so there's nothing extra to install, and `setup-node` caches it ([setup-node](https://github.com/actions/setup-node)).

## Checked, and not yet

- **Checked:** versions and licences from the npm registry on 2026-11-07. Peer ranges: `@vitejs/plugin-react` 6.2.1 wants Vite 8, Vitest 5.1.0 accepts Vite 8 and Node 24, and `vitest-browser-react` 2.3.0 accepts Vitest 5. Every other claim is linked above.
- **Not yet:** nothing was installed or run, because this is a docs PR. Part C's first PR proves, in CI on both systems: that `tsc --noEmit` type-checks the UI with TypeScript 7; that a deliberately failing test names itself and its error in the CI log (S66); that `npx playwright install` needs nothing more on the Windows and macOS runners (the docs only show `--with-deps`, which installs system packages); and that a click test fails when the click doesn't act (S31b).
- **Build target:** Vite 8 targets Chrome 111 and Safari 16.4 by default ([build](https://vite.dev/guide/build)). Tauri's example uses `chrome105` and `safari13`. Which WebKit a given macOS's WKWebView has isn't checked here, and the oldest macOS Gathr supports isn't decided ([0016](0016-macos.md) doesn't name one). When it is, `build.target` follows it.

## Turned down

- **ESLint with typescript-eslint, and Prettier.** That's two tools and more configuration where Biome is one. typescript-eslint's typed rules call the TypeScript API, which TypeScript 7 doesn't have. The TypeScript team's advice is to install TypeScript 6 alongside for such tools ([TypeScript 7.0](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/)). typescript-eslint's docs don't mention TypeScript 7 ([typescript-eslint](https://typescript-eslint.io/getting-started/)). Biome's type-aware `noFloatingPromises` catches about 75% of what typescript-eslint's does ([Biome v2](https://biomejs.dev/blog/biome-v2/)). That's the gap accepted, and `tsc` checks the types.
- **Oxlint.** A linter only, so a formatter would still be needed beside it.
- **Vitest with jsdom or happy-dom.** These are a simulated DOM: no layout, focus or real events. That's the kind of test that passes while the click does nothing.
- **Playwright Test on its own.** It's a second runner for the same job. Vitest's browser mode already drives Playwright.
- **End-to-end tests of the Tauri window (WebdriverIO and `@wdio/tauri-service`).** It works on Windows and macOS ([Tauri WebDriver](https://v2.tauri.app/develop/tests/webdriver/)). But the window holds no state ([0012](0012-ui-shell.md)): every screen is the browser UI, tested above. What only the window does (tray, notifications, open at login) is a hand check for now. Revisit when a window-only feature needs a test.
- **pnpm, Yarn, Bun.** Tauri's scaffold offers each ([create a project](https://v2.tauri.app/start/create-project/)), but npm already comes with Node and one UI package needs no workspaces.
- **Node 26.** It's the newer LTS, but would mean an install on this PC for no gain to a build tool. Move when 24 nears its end of life.
- **TypeScript 6.** It's slower and has no gain here, since nothing needs its API.

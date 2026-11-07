# 0016. Using one computer's chats from another, over Tailscale

Status: proposed. Date: 2026-11-07. Tailscale is the owner's pick.

The owner works from a Windows PC and a Mac, and wants to use the chats on one from the other, both ways.

## Choice

- **Chats stay on the computer that runs them.** Each computer runs its own `gathrd` with its own engines, logins, files and database. Using the other computer means opening its Gathr, not syncing chats between them. This is how Claude Code's Remote Control and the ChatGPT desktop app treat local work: the session keeps running on its own machine and the other device is "a window into that local session" ([Remote Control](https://code.claude.com/docs/en/remote-control)); "Local conversations stay on your computer" ([ChatGPT release notes](https://help.openai.com/en/articles/6825453-chatgpt-release-notes)).
- **The connection is the person's own Tailscale network.** Remote use is off by default. Turning on **Other computers** in Settings runs `tailscale serve` for `gathrd`'s port, so the service is reachable over HTTPS at the computer's Tailscale name, only from the person's own devices ([tailscale serve](https://tailscale.com/kb/1312/serve)). `gathrd` itself keeps listening on `127.0.0.1` only.
- **Each other computer is paired once.** The host shows a short code; the other computer's Gathr enters it and gets its own device token, stored on that computer only. Settings lists paired devices, with Remove. Every remote request needs a device token, and the Tailscale login on the request (`Tailscale-User-Login`, set by `tailscale serve`) must be the one the host was paired under.
- **In the window, computers are a switch.** Gathr on the Mac lists "This Mac" and the paired PC; picking the PC shows the PC's Gathr, served by the PC's own service, in the same window. A browser tab on a paired device works too, through the same one-time-link exchange 0004 uses.
- **What changes in 0004.** The service accepts, besides `127.0.0.1` and `localhost`, the host's Tailscale name as `Host` and its HTTPS origin as `Origin`, only while Other computers is on. Every remote request is recorded in the event record with its device.
- **SSH, documented, no code.** A person who already runs SSH can forward the port (`ssh -L <port>:127.0.0.1:<port> <host>`) and pair the same way; requests then arrive as local ones. The docs say how; Gathr adds nothing for it.

## Why

- **Free, nothing hosted, encrypted end to end.** Tailscale's Personal plan is free for up to six users ([free plans](https://tailscale.com/docs/account/manage-plans/free-plans-discounts.md)), runs on Windows and macOS, and connects devices directly over WireGuard. Gathr runs no server, which "No paid services required" asks for.
- **Nothing is opened to the network.** `tailscale serve` proxies the localhost port to the person's tailnet only; it isn't public (that's Funnel, a separate feature). No firewall rule, no admin rights, no port 22.
- **The identity headers can be trusted only this way.** `tailscale serve` sets `Tailscale-User-Login` and strips any copy the client sent, but anyone who reaches the port directly could forge it, so Tailscale's advice is to keep the service on localhost (same page). That's what `gathrd` already does; the device token is still required, so the header is a second check, not the only one.
- **A device token, not a shared one.** The local token in `service.json` stays on its computer. A token per device can be removed alone if a laptop is lost.
- **The owner named it.** They suggested Tailscale or SSH. Tailscale works away from home; SSH needs a server enabled (on Windows, an admin install that opens port 22 ([OpenSSH on Windows](https://learn.microsoft.com/en-us/windows-server/administration/openssh/openssh_install_firstuse))) and reaches the other computer only on the same network unless something like Tailscale is there anyway.

## How the others do it (2026-11-07)

- **Claude Code, Remote Control:** the local session "makes outbound HTTPS requests only and never opens inbound ports"; traffic goes through Anthropic's API over TLS, and any device signed in to the same claude.ai account can open it. Needs a Pro, Max, Ultra, Team or Enterprise plan; not API keys ([Remote Control](https://code.claude.com/docs/en/remote-control)). The desktop app also has SSH environments ([desktop](https://code.claude.com/docs/en/desktop)).
- **Codex in the ChatGPT desktop app:** phones pair with a host by QR code; "a secure relay layer keeps trusted machines reachable… without exposing them directly". It can't be set up from the Codex CLI. For SSH hosts the app starts a remote app-server over SSH, and the docs advise a VPN or mesh network rather than exposing app-server ([remote connections](https://learn.chatgpt.com/docs/remote-connections.md)).
- **VS Code Remote Tunnels:** outbound connections to Microsoft's relay, both ends signed in with the same GitHub or Microsoft account, SSH inside the tunnel ([tunnels](https://code.visualstudio.com/docs/remote/tunnels)).

All three use a relay their company runs, signed in with the company's account. Gathr has no company service to sign in to, which is why it uses the person's own network.

## Turned down

- **A relay Gathr would run** (the Claude, Codex and VS Code way). Works anywhere with no install, but someone has to host and pay for it, and every message passes through it. Kept as a possible opt-in later, never required.
- **Each provider's own remote feature.** Claude's Remote Control reaches one Claude session from claude.ai, not Gathr's chats, rooms or other agents; Codex's works only from the ChatGPT app. Neither is generic across agents.
- **Listening on the home network with Gathr's own TLS and pairing.** Works only at home, and means Gathr manages certificates and a port on the LAN, which Tailscale does already.
- **Embedding Tailscale (`tsnet`).** It's a Go library ([tsnet](https://pkg.go.dev/tailscale.com/tsnet)), so it would mean shipping a Go sidecar inside a Rust app, for the gain of not installing Tailscale.
- **Syncing chats between computers.** The engines, logins and project files are on one computer; a synced chat couldn't run on the other.

## The owner's calls

- **Tailscale (2026-11-07).** Asked to choose between Tailscale, SSH only, or a relay later, the owner chose Tailscale. Turned down: SSH as the main way (kept as documented, with no code), and a relay.

## Not verified

- `tailscale serve` on Windows run by Gathr as the signed-in user, without admin, and on the Mac App Store and standalone builds of Tailscale. Checked on both computers when remote use is built.
- That WebSocket upgrades pass through `tailscale serve` with their `Origin` intact.
- Latency of a streamed reply from the PC shown on the Mac over Tailscale; measured when built.

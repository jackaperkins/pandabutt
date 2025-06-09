# Pandabutt

Let's use [p2panda.org](https://p2panda.org/) to create a proof-of-concept quality app modelled after a simple version of the [SSB (Secure Scuttlebutt)](https://scuttlebutt.nz/) social network.

## Rationale
SSB is a relatively simple model to base a proof-of-concept app on. In SSB peers have a single append-only log of operations like post, follow, block. There was however no fixed schema for the operations and many non-blog style applications were available in parallel. When syncing peers generally agreed to replicate operations from other peers so long as they were friends of their own friends. This "two-hop" distance protocol promoted network health and some simple discoverability while also preventing a lot of spam.

## Next Steps
- [x] Show pubkey-based identities in frontend
- [x] Rework app database to use sqlite with proper tables
- [ ] Add UI for creating follow operations
- [ ] Sync correctly based on following

## Architecture

### Operation
- **post** `string` - Messages posted by users
- **follow** `string` - One-way following link, value is the public key of target
- **about** `{name: string, avatar: string of base64 encoded image}` - Self-identification for user. Stretch goal for now

### Syncing
should follow SSB friend-of-friend as topic query

### UI
Currently a local web server serving a simple html+js page. Final UI should have a single feed of eveyones posts with a sidebar for your own posts. And a way to follow a given key, especially since we will have to manually bootstrap our first friend in the network without pubs or anything.

Eventually I would like to rework the UI to use [gtk-rs](https://gtk-rs.org/) and make a proper native desktop app. Though that is more an exercise for learning GTK than p2p technologies.

Current look:
![pandabutt screenshot](/pandabutt.jpg)
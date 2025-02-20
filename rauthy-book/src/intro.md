# Introduction

```admonish caution
There has not been any third party security audit for this project.  
Use this software at your own risk!
```

```admonish info
This project is currently pre v1.0, which means, even though it is not expected, breaking changes might come
with new versions.
```

## What it is

Rauthy is an OpenID Connect (OIDC) Provider and Single Sign-On solution written in Rust.

**Secure by default**  
It tries to be as secure as possible by default while still providing all the options needed to be compatible with
older systems. For instance, if you create a new OIDC client, it activates `ed25519` as the default algorithm for
token signing and S256 PKCE flow. This will not work with old clients, which do not support it, but you can of course
deactivate this to your liking.

**MFA and Passwordless Login**  
Rauthy provides FIDO 2 / Webauthn login flows. If you once logged in on a new client with your username + password, you
will get an encrypted cookie which will allow you to log in without a password from that moment on. You only need to
have a FIDO  compliant Passkey being registered for your account.  
The reason why it requests your password on a new host at least once is pretty simple. Even though most browsers have
full support even for user verification, it is possible that in some scenarios a set PIN oder biometric fingerprint
reader will just be ignored by some browsers, which would reduce the strong MFA login to only a single factor again.
As long as the full support does not exist on every device out there, Rauthy will not allow a "Passkey only Login flow"
for security reasons.  
An Example for a not correctly working combination would be Firefox on Mac OS, Firefox pre v114 on Linux, or almost
every browser on Android.

**Fast and efficient**  
The main goal was to provide an SSO solution like Keycloak and others while using a way lower footprint
and being more efficient with resources. For instance, Rauthy can easily run a fully blown SSO provider on just a 
Raspberry Pi. It makes extensive use of caching to be as fast as possible in cases where your database is further
away or just a bit slower, because it is maybe running on an SBC from an SD card. Most things are even cached 
for several hours (config options will come in the future) and special care has been taken into account in case of cache
eviction and invalidation.<br />
A Rauthy deployment with the embedded SQLite, filled caches and a small set of clients and users configured typically
only uses **between 15 and 20 MB of memory**! This is pretty awesome when comparing it to other existing solutions
out there. If a password from a login is hashed, the memory consumption will of course go up way higher than this
depending on your configured Argon2ID parameters, which you got fully under control. If you use it with an external
Postgres, the memory consumption of Rauthy itself will even be a bit lower, since it does not need to care about SQLite.
<br />
For achieving this speed and efficiency, some additional design tradeoffs werde made. For instance, some things you
configure statically via config file and not dynamically via UI, while most of them are configured once and then never
touched again.

**Highly Available**  
Even though it makes extensive use of caching, you can run it in HA mode. It uses its own embedded distributed HA cache
called [redhac](https://crates.io/crates/redhac), which cares about cache eviction on remote hosts.
You can choose between a *SQLite* for single instance deployments and a *Postgres*, if you need HA. MySQL support might 
come in the future.

**Client Branding**  
You have a simple way to create some kind of branding or stylized look for the Login page for each client.  
The whole color theme can be changed and each client can have its own custom logo.  
Additionally, if you modify the branding for the default `rauthy` client, it will not only change the look for the Login
page, but also for the Account and Admin page.

**Already in production**  
Rauthy is already being used in production, and it works with all typical OIDC clients (so far). It was just not an 
open source project for quite some time.  
Keycloak was a rough inspiration in certain places and if something is working with Keycloak, it does with `rauthy` too
(again, so far).

## What it is not (yet?)

Since Rauthy is currently pre v1.0, it might be missing some nice to have features. Some of them will never be
implemented (see below), while others might come or are even planned already.

Currently missing features:

**Rauthy Authenticator MFA App**  
Even though things like OTP codes will never be implemented, it is not set in stone yet that there will never be Rauthy's
own Authenticator App, which then basically acts as a Webauthn Software Authenticator. There are already existing 
solutions out there to serve this purpose.  

**OIDC Client**  
Rauthy will most probably have the option to be an OIDC Client itself as well. With this feature, you would be able
to do things like "Login with Github" to Rauthy and then use Rauthy for the extended management and features.

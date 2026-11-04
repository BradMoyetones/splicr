# Security

## Reporting a vulnerability

Please **don't open a public issue**. Report it privately through GitHub:
[Report a vulnerability](https://github.com/BradMoyetones/splicr/security/advisories/new).

Say what you found, how to reproduce it (a file or the steps), and what an attacker could do with
it. You'll get an answer within a week. Once a fix is out, the advisory is published, crediting you
unless you prefer otherwise.

## What matters most

Splicr opens files people receive from others and updates itself from the internet, so these are
the areas where a problem is most serious:

- **Reading files:** a CSV, spreadsheet or zip archive that crashes Splicr, makes it use unbounded
  memory or time, or reads or writes outside what the user chose.
- **Writing files:** an export that overwrites a file without asking, or leaves a damaged one.
- **Updates:** anything that makes Splicr install a package that isn't signed with the project's
  key, or install it somewhere it shouldn't.

## Supported versions

Only the latest release gets fixes. Splicr updates itself, so the fix reaches everyone through the
next release.

## How releases are protected

Every package is signed with the project's ed25519 key in the release workflow, and Splicr checks
that signature and the SHA-256 before installing an update. Only the maintainer can publish a
release.

## Generate UUIDs
Create universally unique identifiers: v4 (fully random) or v7 (start with a timestamp, so sortable and well suited to databases). Generated with your device's cryptographic random number generator, nothing sent online.

## How it works
1. Pick the version and count.
2. Adjust case and hyphens if needed.
3. Click Run and copy the list.

## FAQ
### What is the difference between v4 and v7?
v4 is fully random. v7 starts with the creation time, so later UUIDs sort after earlier ones, which speeds up database indexes.
### Can two UUIDs be identical?
In practice no: with 122 random bits the probability is negligible.
### Can I use one as a secret?
No, a UUID is an identifier, not a password: use the password generator.

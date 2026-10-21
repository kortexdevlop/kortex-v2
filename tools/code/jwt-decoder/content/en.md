## Read a JWT without sending it anywhere
Paste a login token (JWT) to see what it holds: the algorithm, the claims it carries, when it was issued and when it expires. Decoding happens on your device: the token never leaves your computer or phone, which matters because a token often gives access to an account.

## How it works
1. Paste the token (with or without the word "Bearer" in front).
2. In Medium mode, choose whether to pretty-print the JSON.
3. Run the decoder and read the header, the payload and the dates.

## FAQ
### Is my token sent anywhere?
No. Decoding is local, with no network request.
### Does the tool verify the signature?
No. Verifying a signature needs the service's secret or public key: this tool reads a token, it does not prove it is authentic.
### Why is an "encrypted" token not shown?
An encrypted token (JWE) hides its content: you need the key to read it, here or anywhere else.
### What do the dates mean?
"Issued at" is the `iat` claim and "Expires at" is `exp`, shown in UTC.

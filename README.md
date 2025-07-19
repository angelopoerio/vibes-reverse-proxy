# About
This is a small experiment with a 100% vibe coded project using the [Gemini AI agent of Google](https://github.com/google-gemini/gemini-cli).
It is a simple http [reverse proxy](https://en.wikipedia.org/wiki/Reverse_proxy) in [Rust](https://www.rust-lang.org/). The code was produced as result of a one hour interaction with the agent!

# How to run
```bash
cargo build
REMOTE_HOST=www.google.com:80/ target/debug/vibes-reverse-proxy
```

the environment variable **REMOTE_HOST** is used to set the full URL of the server where your requests should be routed

# Important things
The code is not polished or nearly production ready. It misses all the features of a proper software projects like unit tests, proper observability etc.
The agent decided which libraries to use and which approach to use to structure the code and handle the errors. No manual edits to the code!
It kinda works (a lot of stuff missings) but I have run out of free token for today so no more AI interaction :)

**DON'T USE THIS IN PRODUCTION**

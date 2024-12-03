# Example program with plugin support for Highload++ 2024.

[Google slides](https://docs.google.com/presentation/d/1dXDry0LFU2lJ338txGoEC-UISs7Trp3AHXwAYyla0BI/edit#slide=id.g31a87712e99_0_235)

The program implements a simple console interface. You may want to use `rlwrap`
for better UX:
```sh
sudo apt install rlwrap
```

Build
```sh
cargo build --all
```

Run the host program (`rlwrap` optional).
```sh
rlwrap cargo run -p host-program
```

Run interact with the console...
```
> help
```

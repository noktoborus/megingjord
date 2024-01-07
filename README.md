# Build native

```
$ cargo build
```

Run:

```
$ cargo run
```

# Build for WASM32

```
$ trunk build
```

Run:

```
$ trunk serve
```

# Build for Android

```
$ cd megingjord-android
$ cargo ndk --target arm64-v8a -o java/app/src/main/jniLibs/ build --profile release
$ cargo ndk --target x86-64 -o java/app/src/main/jniLibs/ build --profile release
$ cd java
$ ./gradlew build
```

outputs in `megingjord-android/java/app/build/outputs/apk/debug`

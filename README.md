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

# Build for Android (not work correcty now)

```
$ cd megingjord-android
$ cargo ndk --target x86_64 --target x86 --target arm64-v8a --target armeabi-v7a -o java/app/src/main/jniLibs/ build --profile release
$ cd java
$ ./gradlew build
```

outputs in `megingjord-android/java/app/build/outputs/apk/debug`

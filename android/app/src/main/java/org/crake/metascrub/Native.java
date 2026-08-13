package org.crake.metascrub;

/**
 * The boundary with the Rust core. Everything metascrub actually does happens
 * behind these two calls; this class only loads the library and names them.
 *
 * <p>The names must match the {@code #[no_mangle]} exports in
 * {@code crates/metascrub-android/src/lib.rs}
 * ({@code Java_org_crake_metascrub_Native_sanitize} and {@code ..._reportJson}).
 * Changing a package, class or method name without changing the Rust is an
 * {@code UnsatisfiedLinkError} the first time it is called.
 */
final class Native {
    static {
        // The stripped ELF built by cargo-ndk into jniLibs/<abi>/.
        System.loadLibrary("metascrub_android");
    }

    /**
     * Return the cleaned bytes. Throws RuntimeException if the file could not be
     * parsed. The two flags keep more than the safe minimum: the ICC colour
     * profile, and a minimal EXIF orientation tag. Both are off by default.
     */
    static native byte[] sanitize(byte[] input, boolean keepColour, boolean keepOrientation);

    /** Return a JSON report (assurance, format, removed/retained/warnings) — no metadata values. */
    static native String reportJson(byte[] input, boolean keepColour, boolean keepOrientation);

    /**
     * Reduce a photograph's sensor fingerprint and return the re-encoded JPEG.
     * Reaches into the pixels; reduces linkability, does not remove it. Throws
     * if the bytes are not a decodable image. Strength: 0 gentle, 1 balanced,
     * 2 thorough.
     */
    static native byte[] reduceFingerprint(byte[] input, int strength);

    private Native() {}
}

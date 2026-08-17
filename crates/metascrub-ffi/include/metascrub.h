/*
 * metascrub C ABI — bytes in, cleaned bytes (or a JSON report) out.
 *
 * Backed by the pure-Rust metascrub core (see crates/metascrub-ffi). Every
 * buffer returned here is owned by the caller and must be freed with the
 * matching function — ms_buffer_free for MsBuffer, ms_string_free for a report
 * string — and nothing else. Input pointers are always borrowed, never retained.
 *
 * The report JSON never contains a metadata *value*: only categories, structural
 * locations and counts. The shape matches the Android bridge, so every front end
 * parses one contract.
 */
#ifndef METASCRUB_H
#define METASCRUB_H

#include <stddef.h>
#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/* An owned byte buffer. On failure `data` is NULL and `len` is 0. */
typedef struct MsBuffer {
    uint8_t *data;
    size_t len;
} MsBuffer;

/*
 * Clean `input` and return the sanitized bytes. `data` is NULL on error (a
 * format claimed but unparseable). A format the core cannot take apart comes
 * back unchanged; read assurance == "none" from ms_report_json and warn rather
 * than presenting it as cleaned. Free with ms_buffer_free.
 */
MsBuffer ms_sanitize(const uint8_t *input, size_t len,
                     bool keep_colour, bool keep_orientation);

/*
 * Inspect `input` and return a JSON report without rebuilding the file. On any
 * error the JSON is {"error":"..."}. Free with ms_string_free.
 */
char *ms_report_json(const uint8_t *input, size_t len,
                     bool keep_colour, bool keep_orientation);

/*
 * Reduce a photo's sensor fingerprint (denoise, downscale, add noise, re-encode)
 * and return the re-encoded JPEG. REDUCES linkability, does not remove it.
 * strength: 0 gentle, 1 balanced, 2 thorough. `data` is NULL on error. Run the
 * result through ms_sanitize too, so no metadata rides the new JPEG. Free with
 * ms_buffer_free.
 */
MsBuffer ms_reduce_fingerprint(const uint8_t *input, size_t len, int32_t strength);

/*
 * Convert a photo (JPEG/PNG/WebP) to a metadata-free PNG, re-encoded from raw
 * pixels — the "render path" for things like avatars, where a user may pick a
 * JPEG but it should be stored and shown as a clean PNG. Drops all source
 * metadata, preserves alpha, and downscales so the longest edge is at most
 * `max_edge` (0 keeps the original size; a small image is never enlarged). This
 * is a format conversion plus a scrub, NOT fingerprint reduction — use
 * ms_reduce_fingerprint for that. `data` is NULL on error. Free with
 * ms_buffer_free.
 */
MsBuffer ms_to_png(const uint8_t *input, size_t len, uint32_t max_edge);

/* Free a buffer from ms_sanitize / ms_reduce_fingerprint / ms_to_png. NULL-data is a no-op. */
void ms_buffer_free(MsBuffer buf);

/* Free a string from ms_report_json. NULL is a no-op. */
void ms_string_free(char *s);

#ifdef __cplusplus
}
#endif

#endif /* METASCRUB_H */

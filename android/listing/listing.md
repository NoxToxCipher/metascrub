# Store listing

The text a store shows. Kept here rather than typed into a web form once, so it
can be reviewed like anything else and so the next store gets the same words.

Field lengths follow the usual convention (a 30-character name, an 80-character
short description, 4000 for the full one). Trim to fit if a store asks for less.

---

## App name

    metascrub

## Short description

    Removes what a file says about you, and tells you what it found.

## Full description

metascrub removes the information a photo or a document carries about you, and
tells you exactly what it found.

A photograph carries a second payload that has nothing to do with what it looks
like: the coordinates of where it was taken, the serial number of the camera
that took it, a name in the copyright field, and blocks of vendor data that
nobody documents. A document carries its author, its editing history, and the
identifiers that tie two files to one machine. Send it and you send all of that.

Share a photo or a document to metascrub, or open the app and choose one. It
shows you what the file carries, and writes a cleaned copy only when you ask for
it. Your original is never changed.

Everything happens on your phone.

**No permissions, not one.** Not internet, not storage, not location, not
camera. The app cannot send your file anywhere because it was never given the
ability to, and you can check that for yourself in the app's permission list
before you trust a word of this.

**Three results, and no green tick that was not earned.**

COMPLETE. The file was taken apart and rebuilt from a list of the parts worth
keeping, so anything the tool has never seen, including private or deliberately
hidden blocks, was left behind by construction rather than by being recognised
and deleted. Nothing unknown survives that.

BEST EFFORT. The file was edited in place instead of rebuilt, because rebuilding
it would risk making it unopenable. Identifying fields are gone, and some
structure that cannot safely be touched may remain.

NOT CLEANED. metascrub could not take this format apart, so it left the file
alone and tells you to assume it still carries everything. A result you can
trust is worth more than a tick that was not earned.

**What it cleans.** Photos: JPEG, PNG, WebP, HEIC, AVIF, GIF and TIFF. Documents:
PDF, Word, Excel, PowerPoint and OpenDocument, including the photos inside them.
Camera raw from most brands, cleaned in place so the file still opens. Video and
audio are a different matter. The app names them and tells you what they leak,
but it cannot clean them yet and will not pretend it has.

**What it cannot do.** It cannot make an upload anonymous: cleaning handles what
is inside the file, and a site you are logged in to keeps its own record of who
sent what. It cannot remove the fingerprint a camera sensor leaves in every
photo, though there is a setting that reduces how well that fingerprint matches.
It cannot help with a format it does not understand, and it will tell you so.

A handbook inside the app explains where this information comes from, which
files carry the most of it, and several widely repeated pieces of advice that
are wrong.

Free and open source, GPL-3.0. No accounts, no telemetry, no adverts, and
nothing to buy. Available in Arabic, Belarusian, Burmese, English, Esperanto,
Kurdish (Sorani and Kurmanji), Latin, Persian, Russian and Ukrainian.

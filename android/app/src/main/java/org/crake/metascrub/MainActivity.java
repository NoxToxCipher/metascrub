package org.crake.metascrub;

import android.app.Activity;
import android.app.AlertDialog;
import android.content.ClipData;
import android.content.Context;
import android.content.Intent;
import android.content.res.Configuration;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.net.Uri;
import android.os.Bundle;
import android.provider.DocumentsContract;
import android.provider.OpenableColumns;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.View;
import android.widget.Button;
import android.widget.CheckBox;
import android.widget.LinearLayout;
import android.widget.RadioGroup;
import android.widget.ScrollView;
import android.widget.TextView;
import android.widget.Toast;

import org.json.JSONArray;
import org.json.JSONObject;

import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.security.SecureRandom;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;

/**
 * The whole app. Files are added to a queue (shared, opened, or picked), the
 * queue is reviewed, and only when Scrub is pressed is anything read or cleaned.
 * A second tab holds the Handbook.
 *
 * <p>Nothing is uploaded and nothing is kept: bytes live in memory for as long
 * as one screen needs them, and the only thing written to disk is a cleaned copy
 * the user explicitly saves. There is no network code and no permission for any.
 */
public class MainActivity extends Activity {

    private static final int REQ_PICK = 1;
    private static final int REQ_SAVE_ONE = 2;
    private static final int REQ_SAVE_TREE = 3;

    // A file is held in memory and copied again across the JNI boundary, so a cap
    // keeps a very large file from crashing the app. Above a large RAW photo.
    private static final long MAX_BYTES = 100L * 1024 * 1024;

    private static final int MATCH = LinearLayout.LayoutParams.MATCH_PARENT;
    private static final int WRAP = LinearLayout.LayoutParams.WRAP_CONTENT;
    private static final int PILL_TEXT = 0xFF0A1416;
    private static final int TRANSPARENT = 0x00000000;

    /** One file in the queue, plus its result once scrubbed. */
    private static final class Item {
        final Uri uri;
        final String name;
        final long size;
        final String mime;
        // Filled in by scrubAll():
        boolean done;
        boolean writable;
        boolean foundLocation;
        int removedCount;
        String removedList = "";
        // Identifying data knowingly left in the file, each {what, reveals}. Kept
        // colour profiles and raw residue land here; surfacing it is the whole
        // point of the report, so it must never stay silent.
        final List<String[]> retained = new ArrayList<>();
        String assurance = "none";
        String note = "";

        Item(Uri uri, String name, long size, String mime) {
            this.uri = uri;
            this.name = name;
            this.size = size;
            this.mime = mime;
        }
    }

    private final List<Item> queue = new ArrayList<>();
    private boolean scrubbed = false;
    private boolean onHandbook = false;
    private boolean handbookBuilt = false;
    private Item pendingSaveItem;

    // The language chosen in-app, held in memory only so the choice leaves no
    // trace on disk; it resets to the default on a cold start. null = English.
    private static String appLangCode = null;

    // Changing language recreates the Activity to reload every resource, which
    // would otherwise discard the queue. The file list is stashed here (in memory
    // only, like the language) across that single recreate and re-added in
    // onCreate. Scrub results are not carried; the files simply return unscrubbed.
    private static List<Uri> restoreUris = null;

    private LinearLayout findings;
    private LinearLayout handbookContainer;
    private LinearLayout scrubTab;
    private ScrollView handbookTab;
    private Button btnPrimary;
    private Button btnSecondary;
    private CheckBox optRandom;
    private CheckBox optKeepColour;
    private CheckBox optKeepOrientation;
    private CheckBox optFingerprint;
    private View fingerprintDetail;
    private RadioGroup strengthGroup;
    private TextView langChip;
    private LinearLayout tabScrub;
    private LinearLayout tabHandbook;
    private TextView tabScrubLabel;
    private TextView tabHandbookLabel;
    private View tabScrubRule;
    private View tabHandbookRule;

    /** Apply the in-app language choice by wrapping the context's resources. */
    @Override
    protected void attachBaseContext(Context base) {
        super.attachBaseContext(localized(base));
    }

    private static Context localized(Context base) {
        if (appLangCode == null || "en".equals(appLangCode)) return base;
        Configuration cfg = new Configuration(base.getResources().getConfiguration());
        cfg.setLocale(toLocale(appLangCode));
        return base.createConfigurationContext(cfg);
    }

    private static Locale toLocale(String code) {
        // Central Kurdish (Sorani) is written in the Arabic script and is
        // right-to-left; naming the script makes the layout mirror even where
        // the system would not infer direction from the language code alone.
        if ("ckb".equals(code)) {
            return new Locale.Builder().setLanguage("ckb").setScript("Arab").build();
        }
        return new Locale(code);
    }

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        setContentView(R.layout.activity_main);

        findings = findViewById(R.id.findings_container);
        handbookContainer = findViewById(R.id.handbook_container);
        scrubTab = findViewById(R.id.scrub_tab);
        handbookTab = findViewById(R.id.handbook_tab);
        btnPrimary = findViewById(R.id.btn_primary);
        btnSecondary = findViewById(R.id.btn_secondary);
        optRandom = findViewById(R.id.opt_random);
        optKeepColour = findViewById(R.id.opt_keep_colour);
        optKeepOrientation = findViewById(R.id.opt_keep_orientation);
        optFingerprint = findViewById(R.id.opt_fingerprint);
        fingerprintDetail = findViewById(R.id.fingerprint_detail);
        strengthGroup = findViewById(R.id.strength_group);
        langChip = findViewById(R.id.lang_chip);

        // The strength choice only matters when fingerprint reduction is on.
        optFingerprint.setOnCheckedChangeListener((b, checked) ->
                fingerprintDetail.setVisibility(checked ? View.VISIBLE : View.GONE));

        // Keep-colour and keep-orientation change what the report will say, so a
        // change after scrubbing sends the queue back to be scrubbed again — the
        // shown result never disagrees with what a save would write. (Random names
        // and fingerprint reduction only affect the save, not the report.)
        optKeepColour.setOnCheckedChangeListener((b, c) -> invalidateScrub());
        optKeepOrientation.setOnCheckedChangeListener((b, c) -> invalidateScrub());
        tabScrub = findViewById(R.id.tab_scrub);
        tabHandbook = findViewById(R.id.tab_handbook);
        tabScrubLabel = findViewById(R.id.tab_scrub_label);
        tabHandbookLabel = findViewById(R.id.tab_handbook_label);
        tabScrubRule = findViewById(R.id.tab_scrub_rule);
        tabHandbookRule = findViewById(R.id.tab_handbook_rule);

        btnPrimary.setOnClickListener(v -> onPrimary());
        btnSecondary.setOnClickListener(v -> onSecondary());
        tabScrub.setOnClickListener(v -> switchTab(false));
        tabHandbook.setOnClickListener(v -> switchTab(true));
        langChip.setText(appLangCode == null ? "EN" : appLangCode.toUpperCase(Locale.ROOT));
        langChip.setOnClickListener(v -> showLanguageDialog());

        addUris(incomingUris(getIntent()));
        // Re-add anything the queue held before a language recreate. addUris
        // dedups, so a file the app was launched with is not added twice.
        if (restoreUris != null) {
            addUris(restoreUris);
            restoreUris = null;
        }
        render();
    }

    // --- tabs ------------------------------------------------------------

    private void switchTab(boolean handbook) {
        onHandbook = handbook;
        scrubTab.setVisibility(handbook ? View.GONE : View.VISIBLE);
        handbookTab.setVisibility(handbook ? View.VISIBLE : View.GONE);
        tabScrubLabel.setTextColor(color(handbook ? R.color.text_muted : R.color.teal));
        tabScrubLabel.setTypeface(Typeface.DEFAULT, handbook ? Typeface.NORMAL : Typeface.BOLD);
        tabScrubRule.setBackgroundColor(handbook ? TRANSPARENT : color(R.color.teal));
        tabHandbookLabel.setTextColor(color(handbook ? R.color.teal : R.color.text_muted));
        tabHandbookLabel.setTypeface(Typeface.DEFAULT, handbook ? Typeface.BOLD : Typeface.NORMAL);
        tabHandbookRule.setBackgroundColor(handbook ? color(R.color.teal) : TRANSPARENT);
        if (handbook && !handbookBuilt) {
            buildHandbook();
            handbookBuilt = true;
        }
    }

    // Same order as the R.array.languages endonyms. A null code means a language
    // that is listed but not loaded yet (its resources are not bundled).
    private static final String[] LANG_CODES = {"en", "ru", "my", "la", "eo", "uk", "be", "fa", "ar", "ckb", "kmr"};

    private void showLanguageDialog() {
        final String[] langs = getResources().getStringArray(R.array.languages);
        new AlertDialog.Builder(this)
                .setTitle(R.string.language)
                .setItems(langs, (dialog, which) -> {
                    String code = which < LANG_CODES.length ? LANG_CODES[which] : null;
                    if (code == null) {
                        Toast.makeText(this, R.string.lang_more_soon, Toast.LENGTH_LONG).show();
                        return;
                    }
                    String current = appLangCode == null ? "en" : appLangCode;
                    if (!code.equals(current)) {
                        appLangCode = "en".equals(code) ? null : code;
                        // Carry the queued files across the recreate so switching
                        // language does not silently empty the queue.
                        restoreUris = new ArrayList<>();
                        for (Item it : queue) restoreUris.add(it.uri);
                        recreate(); // reload every string and the Handbook in the new language
                    }
                })
                .show();
    }

    // --- queue -----------------------------------------------------------

    /** URIs the app was launched with, from a share, a multi-share, or "open with". */
    private List<Uri> incomingUris(Intent it) {
        List<Uri> out = new ArrayList<>();
        if (it == null) return out;
        String action = it.getAction();
        if (Intent.ACTION_SEND.equals(action)) {
            Uri u = it.getParcelableExtra(Intent.EXTRA_STREAM);
            if (u != null) out.add(u);
        } else if (Intent.ACTION_SEND_MULTIPLE.equals(action)) {
            ArrayList<Uri> us = it.getParcelableArrayListExtra(Intent.EXTRA_STREAM);
            if (us != null) out.addAll(us);
        } else if (Intent.ACTION_VIEW.equals(action)) {
            if (it.getData() != null) out.add(it.getData());
        }
        return out;
    }

    private void addUris(List<Uri> uris) {
        boolean added = false;
        for (Uri uri : uris) {
            if (uri == null) continue;
            boolean dup = false;
            for (Item it : queue) {
                if (it.uri.equals(uri)) { dup = true; break; }
            }
            if (dup) continue;
            queue.add(new Item(uri, displayName(uri), fileSize(uri), safeMime(uri)));
            added = true;
        }
        if (added) {
            scrubbed = false; // new files need scrubbing
            for (Item it : queue) it.done = false;
            switchTab(false);
            render();
        }
    }

    private void removeItem(Item it) {
        queue.remove(it);
        if (queue.isEmpty()) scrubbed = false;
        render();
    }

    /** The content type, or null if a hostile or broken provider throws. */
    private String safeMime(Uri uri) {
        try {
            return getContentResolver().getType(uri);
        } catch (Throwable t) {
            return null;
        }
    }

    /** A metadata-policy change invalidates an existing scrub; go back to the queue. */
    private void invalidateScrub() {
        if (scrubbed) {
            scrubbed = false;
            render();
        }
    }

    // --- actions ---------------------------------------------------------

    private void onPrimary() {
        if (queue.isEmpty()) {
            pickFiles();
        } else if (!scrubbed) {
            scrubAll();
        } else {
            saveFlow();
        }
    }

    private void onSecondary() {
        if (queue.isEmpty()) {
            // hidden in this state
        } else if (!scrubbed) {
            pickFiles();
        } else {
            queue.clear();
            scrubbed = false;
            render();
        }
    }

    private void pickFiles() {
        Intent it = new Intent(Intent.ACTION_OPEN_DOCUMENT);
        it.addCategory(Intent.CATEGORY_OPENABLE);
        it.setType("*/*");
        it.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true);
        startActivityForResult(it, REQ_PICK);
    }

    private void scrubAll() {
        btnPrimary.setEnabled(false);
        btnSecondary.setEnabled(false);
        btnPrimary.setText(R.string.scrubbing);
        // Read the toggles on the UI thread; the report must reflect the same
        // policy the eventual save uses.
        final boolean keepColour = optKeepColour.isChecked();
        final boolean keepOrientation = optKeepOrientation.isChecked();
        // Freeze the metadata toggles while the scrub runs. They change what the
        // report says, but the invalidate-on-change guard only fires once a scrub
        // has finished (scrubbed == true); a toggle flipped mid-scrub would
        // otherwise leave the shown result out of step with what a save writes.
        optKeepColour.setEnabled(false);
        optKeepOrientation.setEnabled(false);
        new Thread(() -> {
            for (Item it : queue) inspect(it, keepColour, keepOrientation);
            runOnUiThread(() -> {
                scrubbed = true;
                btnSecondary.setEnabled(true);
                optKeepColour.setEnabled(true);
                optKeepOrientation.setEnabled(true);
                render();
            });
        }).start();
    }

    /** Read one queued file and record what it carries. Runs off the UI thread. */
    private void inspect(Item it, boolean keepColour, boolean keepOrientation) {
        it.done = true;
        try {
            byte[] bytes = readAll(it.uri);
            String report = Native.reportJson(bytes, keepColour, keepOrientation);
            JSONObject r = new JSONObject(report);
            if (r.has("error")) {
                it.assurance = "none";
                it.writable = false;
                it.note = getString(R.string.read_failed, r.optString("error"));
                return;
            }
            it.assurance = r.optString("assurance", "none");
            it.writable = "complete".equals(it.assurance) || "best_effort".equals(it.assurance);
            it.foundLocation = r.optBoolean("found_location", false);
            it.note = noteFor(it.assurance);
            JSONArray removed = r.optJSONArray("removed");
            it.removedCount = removed == null ? 0 : removed.length();
            LinkedHashSet<String> kinds = new LinkedHashSet<>();
            if (removed != null) {
                for (int i = 0; i < removed.length(); i++) {
                    JSONObject e = removed.optJSONObject(i);
                    if (e != null) kinds.add(e.optString("kind"));
                }
            }
            it.removedList = String.join(", ", kinds);
            it.retained.clear();
            JSONArray retained = r.optJSONArray("retained");
            if (retained != null) {
                for (int i = 0; i < retained.length(); i++) {
                    JSONObject e = retained.optJSONObject(i);
                    if (e != null) {
                        it.retained.add(new String[] {e.optString("what"), e.optString("reveals")});
                    }
                }
            }
        } catch (Throwable t) {
            it.assurance = "none";
            it.writable = false;
            it.removedCount = 0;
            it.retained.clear();
            it.note = getString(R.string.read_failed, String.valueOf(t.getMessage()));
        }
    }

    private void saveFlow() {
        List<Item> savable = new ArrayList<>();
        for (Item it : queue) if (it.writable) savable.add(it);
        if (savable.isEmpty()) {
            Toast.makeText(this, R.string.nothing_to_save, Toast.LENGTH_LONG).show();
            return;
        }
        if (savable.size() == 1) {
            pendingSaveItem = savable.get(0);
            boolean jpeg = willWash(pendingSaveItem);
            Intent it = new Intent(Intent.ACTION_CREATE_DOCUMENT);
            it.addCategory(Intent.CATEGORY_OPENABLE);
            it.setType(jpeg ? "image/jpeg" : "application/octet-stream");
            it.putExtra(Intent.EXTRA_TITLE, outName(pendingSaveItem, jpeg));
            startActivityForResult(it, REQ_SAVE_ONE);
        } else {
            // Pick a destination folder once; every cleaned copy is written into it.
            Intent it = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);
            startActivityForResult(it, REQ_SAVE_TREE);
        }
    }

    @Override
    protected void onActivityResult(int req, int result, Intent data) {
        super.onActivityResult(req, result, data);
        if (result != RESULT_OK || data == null) return;

        if (req == REQ_PICK) {
            List<Uri> uris = new ArrayList<>();
            ClipData clip = data.getClipData();
            if (clip != null) {
                for (int i = 0; i < clip.getItemCount(); i++) {
                    Uri u = clip.getItemAt(i).getUri();
                    if (u != null) uris.add(u);
                }
            } else if (data.getData() != null) {
                uris.add(data.getData());
            }
            addUris(uris);
        } else if (req == REQ_SAVE_ONE && data.getData() != null && pendingSaveItem != null) {
            writeOne(pendingSaveItem, data.getData());
        } else if (req == REQ_SAVE_TREE && data.getData() != null) {
            writeAllToTree(data.getData());
        }
    }

    private void writeOne(Item item, Uri dest) {
        btnPrimary.setEnabled(false);
        final boolean keepColour = optKeepColour.isChecked();
        final boolean keepOrientation = optKeepOrientation.isChecked();
        final boolean fingerprint = optFingerprint.isChecked();
        final int strength = strength();
        new Thread(() -> {
            try {
                byte[] cleaned = process(item, keepColour, keepOrientation, fingerprint, strength);
                try (OutputStream out = getContentResolver().openOutputStream(dest, "wt")) {
                    if (out == null) throw new Exception("no output stream");
                    out.write(cleaned);
                }
                runOnUiThread(() -> {
                    Toast.makeText(this, getResources().getQuantityString(R.plurals.saved_files, 1, 1),
                            Toast.LENGTH_LONG).show();
                    btnPrimary.setEnabled(true);
                });
            } catch (Throwable t) {
                final String msg = String.valueOf(t.getMessage());
                runOnUiThread(() -> {
                    Toast.makeText(this, getString(R.string.save_failed, msg), Toast.LENGTH_LONG).show();
                    btnPrimary.setEnabled(true);
                });
            }
        }).start();
    }

    private void writeAllToTree(Uri treeUri) {
        btnPrimary.setEnabled(false);
        final boolean keepColour = optKeepColour.isChecked();
        final boolean keepOrientation = optKeepOrientation.isChecked();
        final boolean fingerprint = optFingerprint.isChecked();
        final int strength = strength();
        new Thread(() -> {
            int saved = 0;
            String lastError = null;
            Uri dir = DocumentsContract.buildDocumentUriUsingTree(
                    treeUri, DocumentsContract.getTreeDocumentId(treeUri));
            for (Item item : queue) {
                if (!item.writable) continue;
                try {
                    boolean jpeg = fingerprint && isWashable(item);
                    byte[] cleaned = process(item, keepColour, keepOrientation, fingerprint, strength);
                    String mime = jpeg ? "image/jpeg" : (item.mime != null ? item.mime : "application/octet-stream");
                    Uri doc = DocumentsContract.createDocument(
                            getContentResolver(), dir, mime, outName(item, jpeg));
                    if (doc == null) throw new Exception("could not create file");
                    try (OutputStream out = getContentResolver().openOutputStream(doc, "wt")) {
                        if (out == null) throw new Exception("no output stream");
                        out.write(cleaned);
                    }
                    saved++;
                } catch (Throwable t) {
                    lastError = String.valueOf(t.getMessage());
                }
            }
            final int total = saved;
            final String err = lastError;
            runOnUiThread(() -> {
                if (total > 0) {
                    Toast.makeText(this, getResources().getQuantityString(R.plurals.saved_files, total, total),
                            Toast.LENGTH_LONG).show();
                } else {
                    Toast.makeText(this, getString(R.string.save_failed, String.valueOf(err)),
                            Toast.LENGTH_LONG).show();
                }
                btnPrimary.setEnabled(true);
            });
        }).start();
    }

    @Override
    public void onBackPressed() {
        if (onHandbook) {
            switchTab(false);
        } else if (scrubbed) {
            scrubbed = false;
            render();
        } else if (!queue.isEmpty()) {
            queue.clear();
            render();
        } else {
            super.onBackPressed();
        }
    }

    // --- scrub-tab rendering --------------------------------------------

    private void render() {
        findings.removeAllViews();
        if (queue.isEmpty()) {
            showMessage(getString(R.string.share_prompt), color(R.color.text_muted));
            btnPrimary.setText(R.string.choose_files);
            btnPrimary.setEnabled(true);
            btnSecondary.setVisibility(View.GONE);
            return;
        }

        addSectionTitle(getString(scrubbed ? R.string.results_header : R.string.ready_header));
        for (Item it : queue) addItemCard(it);

        btnSecondary.setVisibility(View.VISIBLE);
        if (!scrubbed) {
            btnSecondary.setText(R.string.add_more);
            btnPrimary.setText(getResources().getQuantityString(R.plurals.scrub_files, queue.size(), queue.size()));
            btnPrimary.setEnabled(true);
        } else {
            btnSecondary.setText(R.string.start_over);
            int w = writableCount();
            btnPrimary.setText(getResources().getQuantityString(R.plurals.save_files, Math.max(w, 1), w));
            btnPrimary.setEnabled(w > 0);
        }
    }

    private void addItemCard(Item it) {
        LinearLayout card = card();

        LinearLayout top = box(LinearLayout.HORIZONTAL);
        top.setGravity(Gravity.CENTER_VERTICAL);
        TextView name = text(it.name, 15, color(R.color.text));
        name.setSingleLine(true);
        name.setEllipsize(android.text.TextUtils.TruncateAt.MIDDLE);
        LinearLayout.LayoutParams np = params(0, WRAP);
        np.weight = 1;
        top.addView(name, np);

        if (!scrubbed) {
            TextView remove = text("✕", 15, color(R.color.text_muted)); // ✕
            remove.setPadding(dp(12), dp(4), dp(4), dp(4));
            remove.setContentDescription(getString(R.string.remove));
            remove.setOnClickListener(v -> removeItem(it));
            top.addView(remove);
        } else {
            top.addView(pill(badgeText(it.assurance), badgeColor(it.assurance)));
        }
        card.addView(top);

        if (!scrubbed) {
            addLine(card, humanBytes(it.size) + "  ·  " + ext(it.name), 12.5f, R.color.text_muted, 4);
        } else {
            addLine(card, it.note, 13, R.color.text_muted, 6);
            if (it.foundLocation) {
                addLine(card, getString(R.string.found_location), 13, R.color.danger, 4);
            }
            if (it.removedCount > 0) {
                addLine(card, getString(R.string.removed_prefix, it.removedList), 12.5f, R.color.text_muted, 4);
            } else if (it.writable) {
                addLine(card, getString(R.string.nothing_found), 12.5f, R.color.text_muted, 4);
            }
            // What was knowingly left in, and what it reveals. Framed in the
            // best-effort amber: a clean that keeps something identifying but says
            // nothing is worse than one that spells it out.
            if (!it.retained.isEmpty()) {
                addLine(card, getString(R.string.still_in_file), 12.5f, R.color.warn, 8);
                for (String[] kept : it.retained) {
                    addLine(card, "• " + kept[0], 12f, R.color.warn, 2);
                    if (kept.length > 1 && !kept[1].isEmpty()) {
                        addLine(card, kept[1], 11.5f, R.color.text_muted, 0);
                    }
                }
            }
        }

        LinearLayout.LayoutParams cp = params(MATCH, WRAP);
        cp.topMargin = dp(10);
        findings.addView(card, cp);
    }

    private int writableCount() {
        int n = 0;
        for (Item it : queue) if (it.writable) n++;
        return n;
    }

    // --- handbook-tab rendering -----------------------------------------

    private void buildHandbook() {
        handbookContainer.removeAllViews();
        String json;
        try (InputStream is = getResources().openRawResource(R.raw.handbook)) {
            ByteArrayOutputStream bo = new ByteArrayOutputStream();
            byte[] buf = new byte[8192];
            int n;
            while ((n = is.read(buf)) != -1) bo.write(buf, 0, n);
            json = new String(bo.toByteArray(), StandardCharsets.UTF_8);
        } catch (Exception e) {
            handbookContainer.addView(text(getString(R.string.handbook_unavailable), 15, color(R.color.danger)));
            return;
        }
        try {
            JSONArray chapters = new JSONObject(json).getJSONArray("chapters");
            for (int c = 0; c < chapters.length(); c++) {
                JSONObject ch = chapters.getJSONObject(c);
                addChapter(ch.optString("title"),
                        ch.has("intro") ? ch.optString("intro") : null,
                        ch.optJSONArray("entries"));
            }
        } catch (Exception e) {
            handbookContainer.removeAllViews();
            handbookContainer.addView(text(getString(R.string.handbook_unavailable), 15, color(R.color.danger)));
        }
    }

    /** A collapsible chapter: a tappable bubble that opens its entries below. */
    private void addChapter(String title, String intro, JSONArray entries) {
        LinearLayout header = box(LinearLayout.HORIZONTAL);
        header.setBackgroundResource(R.drawable.bg_card);
        header.setPadding(dp(16), dp(16), dp(16), dp(16));
        header.setGravity(Gravity.CENTER_VERTICAL);
        TextView t = text(title, 16, color(R.color.teal), true);
        LinearLayout.LayoutParams tp = params(0, WRAP);
        tp.weight = 1;
        header.addView(t, tp);
        final TextView chevron = text("▸", 15, color(R.color.teal)); // ▸ collapsed
        header.addView(chevron);
        LinearLayout.LayoutParams hp = params(MATCH, WRAP);
        hp.topMargin = dp(10);
        handbookContainer.addView(header, hp);

        final LinearLayout body = box(LinearLayout.VERTICAL);
        body.setVisibility(View.GONE);
        body.setPadding(dp(6), 0, dp(6), dp(4));
        if (intro != null && !intro.isEmpty()) {
            TextView in = text(intro, 13.5f, color(R.color.text_muted));
            in.setLineSpacing(dp(4), 1f);
            LinearLayout.LayoutParams ip = params(MATCH, WRAP);
            ip.topMargin = dp(12);
            body.addView(in, ip);
        }
        if (entries != null) {
            for (int i = 0; i < entries.length(); i++) {
                JSONObject e = entries.optJSONObject(i);
                if (e == null) continue;
                String heading = e.optString("heading");
                if (!heading.isEmpty()) {
                    TextView h = text(heading, 15, color(R.color.text), true);
                    LinearLayout.LayoutParams hep = params(MATCH, WRAP);
                    hep.topMargin = dp(16);
                    body.addView(h, hep);
                }
                TextView b = text(e.optString("body"), 14, color(R.color.text_muted));
                b.setLineSpacing(dp(4), 1f);
                LinearLayout.LayoutParams bp = params(MATCH, WRAP);
                bp.topMargin = dp(heading.isEmpty() ? 12 : 4);
                body.addView(b, bp);
            }
        }
        handbookContainer.addView(body, params(MATCH, WRAP));

        header.setOnClickListener(v -> {
            boolean show = body.getVisibility() == View.GONE;
            body.setVisibility(show ? View.VISIBLE : View.GONE);
            chevron.setText(show ? "▾" : "▸"); // ▾ / ▸
        });
    }

    // --- shared view helpers --------------------------------------------

    private void showMessage(String message, int textColor) {
        TextView t = text(message, 15, textColor);
        t.setLineSpacing(dp(4), 1f);
        LinearLayout.LayoutParams p = params(MATCH, WRAP);
        p.topMargin = dp(20);
        findings.addView(t, p);
    }

    private void addSectionTitle(String title) {
        TextView t = text(title.toUpperCase(Locale.ROOT), 12, color(R.color.text_muted));
        t.setLetterSpacing(0.1f);
        LinearLayout.LayoutParams p = params(MATCH, WRAP);
        p.topMargin = dp(10);
        findings.addView(t, p);
    }

    private void addLine(LinearLayout card, String s, float sizeSp, int colorRes, int topDp) {
        TextView t = text(s, sizeSp, color(colorRes));
        t.setLineSpacing(dp(3), 1f);
        LinearLayout.LayoutParams p = params(MATCH, WRAP);
        p.topMargin = dp(topDp);
        card.addView(t, p);
    }

    private LinearLayout card() {
        LinearLayout card = box(LinearLayout.VERTICAL);
        card.setBackgroundResource(R.drawable.bg_card);
        card.setPadding(dp(16), dp(14), dp(16), dp(16));
        return card;
    }

    private TextView pill(String label, int fill) {
        TextView t = text(label, 12, PILL_TEXT, true);
        t.setLetterSpacing(0.08f);
        t.setPadding(dp(12), dp(6), dp(12), dp(6));
        GradientDrawable g = new GradientDrawable();
        g.setColor(fill);
        g.setCornerRadius(dp(20));
        t.setBackground(g);
        return t;
    }

    private LinearLayout box(int orientation) {
        LinearLayout l = new LinearLayout(this);
        l.setOrientation(orientation);
        return l;
    }

    private TextView text(String s, float sizeSp, int textColor) {
        return text(s, sizeSp, textColor, false);
    }

    private TextView text(String s, float sizeSp, int textColor, boolean bold) {
        TextView t = new TextView(this);
        t.setText(s);
        t.setTextSize(sizeSp);
        t.setTextColor(textColor);
        if (bold) t.setTypeface(Typeface.DEFAULT_BOLD);
        return t;
    }

    private LinearLayout.LayoutParams params(int w, int h) {
        return new LinearLayout.LayoutParams(w, h);
    }

    private int color(int res) {
        return getColor(res);
    }

    private int dp(float v) {
        return Math.round(TypedValue.applyDimension(
                TypedValue.COMPLEX_UNIT_DIP, v, getResources().getDisplayMetrics()));
    }

    private String noteFor(String assurance) {
        switch (assurance) {
            case "complete": return getString(R.string.note_complete);
            case "best_effort": return getString(R.string.note_best_effort);
            default: return getString(R.string.note_none);
        }
    }

    private String badgeText(String assurance) {
        switch (assurance) {
            case "complete": return getString(R.string.badge_complete);
            case "best_effort": return getString(R.string.badge_best_effort);
            default: return getString(R.string.badge_not_cleaned);
        }
    }

    private int badgeColor(String assurance) {
        switch (assurance) {
            case "complete": return color(R.color.ok);
            case "best_effort": return color(R.color.warn);
            default: return color(R.color.danger);
        }
    }

    // --- file helpers ----------------------------------------------------

    private byte[] readAll(Uri uri) throws Exception {
        try (InputStream in = getContentResolver().openInputStream(uri)) {
            if (in == null) throw new Exception("could not open the file");
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] buf = new byte[64 * 1024];
            int n;
            long total = 0;
            while ((n = in.read(buf)) != -1) {
                total += n;
                if (total > MAX_BYTES) throw new Exception(getString(R.string.too_large));
                out.write(buf, 0, n);
            }
            return out.toByteArray();
        }
    }

    private String displayName(Uri uri) {
        try (android.database.Cursor c = getContentResolver().query(uri, null, null, null, null)) {
            if (c != null && c.moveToFirst()) {
                int i = c.getColumnIndex(OpenableColumns.DISPLAY_NAME);
                if (i >= 0) {
                    String n = c.getString(i);
                    if (n != null && !n.isEmpty()) return n;
                }
            }
        } catch (Exception ignored) {
        }
        return "file";
    }

    private long fileSize(Uri uri) {
        try (android.database.Cursor c = getContentResolver().query(uri, null, null, null, null)) {
            if (c != null && c.moveToFirst()) {
                int i = c.getColumnIndex(OpenableColumns.SIZE);
                if (i >= 0 && !c.isNull(i)) return c.getLong(i);
            }
        } catch (Exception ignored) {
        }
        return -1;
    }

    /**
     * Clean one file to its final bytes. When fingerprint reduction is on and the
     * file is a photo pixelwash can decode, the pixels are washed first (which
     * re-encodes to JPEG), then the result is sanitized so no metadata rides along.
     */
    private byte[] process(Item item, boolean keepColour, boolean keepOrientation,
                           boolean fingerprint, int strength) throws Exception {
        byte[] bytes = readAll(item.uri);
        if (fingerprint && isWashable(item)) {
            byte[] washed = Native.reduceFingerprint(bytes, strength);
            return Native.sanitize(washed, keepColour, keepOrientation);
        }
        // Save re-reads the file, so the bytes here may differ from the ones
        // inspect() judged writable (a racing or hostile content provider can
        // swap them). A format the core cannot take apart comes back unchanged
        // from sanitize, which would write the untouched original as a "cleaned
        // copy". Re-inspect these exact bytes and refuse if they are not clean.
        String report = Native.reportJson(bytes, keepColour, keepOrientation);
        JSONObject r = new JSONObject(report);
        String assurance = r.has("error") ? "none" : r.optString("assurance", "none");
        if (!("complete".equals(assurance) || "best_effort".equals(assurance))) {
            throw new Exception(getString(R.string.save_changed));
        }
        return Native.sanitize(bytes, keepColour, keepOrientation);
    }

    /** Fingerprint reduction re-encodes to JPEG, so it only applies to formats
     *  pixelwash can decode. Other images are still metadata-cleaned. */
    private boolean isWashable(Item item) {
        if (item.mime == null) return false;
        switch (item.mime) {
            case "image/jpeg":
            case "image/png":
            case "image/webp":
                return true;
            default:
                return false;
        }
    }

    /** Strength for pixelwash: 0 gentle, 1 balanced, 2 thorough. */
    private int strength() {
        int id = strengthGroup.getCheckedRadioButtonId();
        if (id == R.id.strength_gentle) return 0;
        if (id == R.id.strength_thorough) return 2;
        return 1;
    }

    /** Whether this item will be washed (and therefore written as JPEG) on save. */
    private boolean willWash(Item item) {
        return optFingerprint.isChecked() && isWashable(item);
    }

    /**
     * Output name: a fresh 24-character random name by default (keeping the
     * extension), or the original name. A washed photo is written as .jpg.
     */
    private String outName(Item item, boolean jpeg) {
        String ext = jpeg ? ".jpg" : extWithDot(item.name);
        if (optRandom.isChecked()) return random24() + ext;
        if (jpeg) return stripExt(item.name) + ".jpg";
        return item.name;
    }

    private String random24() {
        final String alphabet = "abcdefghijklmnopqrstuvwxyz234567";
        SecureRandom rng = new SecureRandom();
        StringBuilder sb = new StringBuilder(24);
        for (int i = 0; i < 24; i++) sb.append(alphabet.charAt(rng.nextInt(alphabet.length())));
        return sb.toString();
    }

    private static String extWithDot(String name) {
        int d = name.lastIndexOf('.');
        return (d > 0 && d < name.length() - 1) ? name.substring(d).toLowerCase() : "";
    }

    private static String stripExt(String name) {
        int d = name.lastIndexOf('.');
        return d > 0 ? name.substring(0, d) : name;
    }

    private static String humanBytes(long b) {
        if (b < 0) return "";
        if (b < 1024) return b + " B";
        double kb = b / 1024.0;
        if (kb < 1024) return String.format(Locale.US, "%.0f KB", kb);
        return String.format(Locale.US, "%.1f MB", kb / 1024.0);
    }

    private static String ext(String name) {
        int dot = name.lastIndexOf('.');
        if (dot > 0 && dot < name.length() - 1) return name.substring(dot + 1).toUpperCase(Locale.ROOT);
        return "";
    }
}

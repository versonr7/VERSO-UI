package com.verso.ui;

import android.app.NativeActivity;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.util.Log;
import android.content.ContentResolver;
import java.io.InputStream;

public class VersoActivity extends NativeActivity {
    private static final String TAG = "VersoActivity";
    private static final int PICK_FILE_REQUEST = 1001;

    // استدعاء من Rust لفتح متصفح الملفات
    public void openFilePicker() {
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.setType("*/*");
        String[] mimeTypes = {
            "application/vnd.android.package-archive",
            "application/octet-stream",
            "application/x-sharedlib"
        };
        intent.putExtra(Intent.EXTRA_MIME_TYPES, mimeTypes);
        startActivityForResult(intent, PICK_FILE_REQUEST);
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode == PICK_FILE_REQUEST && resultCode == RESULT_OK && data != null) {
            Uri uri = data.getData();
            if (uri != null) {
                // قراءة الملف وتمريره إلى Rust
                try {
                    ContentResolver resolver = getContentResolver();
                    InputStream stream = resolver.openInputStream(uri);
                    if (stream != null) {
                        byte[] bytes = new byte[stream.available()];
                        stream.read(bytes);
                        stream.close();
                        // استدعاء الدالة الأصلية لتمرير البيانات
                        nativeOnFilePicked(uri.toString(), bytes);
                    }
                } catch (Exception e) {
                    Log.e(TAG, "Error reading file: " + e.getMessage());
                }
            }
        }
    }

    // دالة أصلية يتم تنفيذها في Rust
    private native void nativeOnFilePicked(String uriString, byte[] fileData);
}

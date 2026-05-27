package com.verso.ui;

import android.app.NativeActivity;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.util.Log;

public class VersoActivity extends NativeActivity {
    private static final String TAG = "VersoActivity";
    private static final int PICK_FILE_REQUEST = 1001;

    // هذه الدالة تُستدعى من Rust عبر JNI لفتح متصفح الملفات
    public void openFilePicker() {
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.setType("*/*"); // جميع أنواع الملفات
        String[] mimeTypes = {"application/vnd.android.package-archive", "application/octet-stream", "application/x-sharedlib"};
        intent.putExtra(Intent.EXTRA_MIME_TYPES, mimeTypes);
        startActivityForResult(intent, PICK_FILE_REQUEST);
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode == PICK_FILE_REQUEST && resultCode == RESULT_OK && data != null) {
            Uri uri = data.getData();
            if (uri != null) {
                // استدعاء الدالة الأصلية في Rust لتمرير URI
                nativeOnFilePicked(uri.toString());
            }
        }
    }

    // دالة أصلية سيتم تنفيذها في Rust
    private native void nativeOnFilePicked(String uriString);
}

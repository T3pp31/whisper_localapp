use std::env;

fn main() {
    // CUDA環境変数の設定
    // CUDAツールキットのパスを環境変数から取得
    if let Ok(cuda_path) = env::var("CUDA_PATH") {
        println!("cargo:rustc-link-search=native={}/lib64", cuda_path);
        println!("cargo:rustc-link-search=native={}/lib", cuda_path);
    }

    // 標準的なCUDAパスも追加
    println!("cargo:rustc-link-search=native=/usr/local/cuda/lib64");
    println!("cargo:rustc-link-search=native=/usr/local/cuda/lib");
    println!("cargo:rustc-link-search=native=/opt/cuda/lib64");
    println!("cargo:rustc-link-search=native=/opt/cuda/lib");

    // CUDA関連のライブラリをリンク
    if env::var("WHISPER_CUBLAS").unwrap_or_default() == "1" {
        println!("cargo:rustc-link-lib=cuda");
        println!("cargo:rustc-link-lib=cublas");
        println!("cargo:rustc-link-lib=curand");
        println!("cargo:rustc-link-lib=cufft");

        // デバッグ用：環境変数を出力
        println!("cargo:warning=WHISPER_CUBLAS is enabled");

        // CUDA対応フラグを設定
        println!("cargo:rustc-cfg=feature=\"cuda\"");
    }

    // OpenCL環境変数の設定
    if env::var("WHISPER_OPENCL").unwrap_or_default() == "1" {
        println!("cargo:rustc-link-lib=OpenCL");
        println!("cargo:warning=WHISPER_OPENCL is enabled");
        println!("cargo:rustc-cfg=feature=\"opencl\"");
    }

    // whisper.cpp のコンパイル時フラグを設定
    if env::var("WHISPER_CUBLAS").unwrap_or_default() == "1" {
        println!("cargo:rustc-env=GGML_USE_CUBLAS=1");
        println!("cargo:rustc-env=WHISPER_CUBLAS=1");
    }

    if env::var("WHISPER_OPENCL").unwrap_or_default() == "1" {
        println!("cargo:rustc-env=GGML_USE_CLBLAST=1");
        println!("cargo:rustc-env=WHISPER_OPENCL=1");
    }

    // ビルド時の情報を表示
    println!("cargo:rerun-if-env-changed=WHISPER_CUBLAS");
    println!("cargo:rerun-if-env-changed=WHISPER_OPENCL");
    println!("cargo:rerun-if-env-changed=CUDA_PATH");
}
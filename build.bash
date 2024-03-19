#!/bin/bash
PLATFORM_PREBUILTS=${HOME}/android/platform/prebuilts
GCC_PREBUILT=${PLATFORM_PREBUILTS}/gcc/linux-x86/host/x86_64-linux-glibc2.17-4.8
LLVM_PREBUILT=${PLATFORM_PREBUILTS}/clang/host/linux-x86/clang-r510928
#LIBUNWIND_DIR=${LLVM_PREBUILT}/lib/clang/18/lib/linux/x86_64
LIBUNWIND_DIR=${PLATFORM_PREBUILTS}/rust/linux-x86/1.75.0/lib/rustlib/x86_64-unknown-linux-gnu/lib
LD_LIBRARY_PATH=${LLVM_PREBUILT}/lib:${LD_LIBRARY_PATH} CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C link-arg=--rtlib=compiler-rt -C link-arg=--sysroot -C link-arg=${GCC_PREBUILT}/sysroot -C link-arg=-B${GCC_PREBUILT}/lib/gcc/x86_64-linux/4.8.3/ -C link-arg=-L${GCC_PREBUILT}/lib/gcc/x86_64-linux/4.8.3/ -C link-arg=-L${GCC_PREBUILT}/x86_64-linux/lib64/ -C link-arg=-L${LLVM_PREBUILT}/lib -C link-arg=-L${LIBUNWIND_DIR} -C link-arg=-Wl,-rpath,\$ORIGIN/lib -C link-arg=-Wl,--disable-new-dtags" ./x.py build --stage=1

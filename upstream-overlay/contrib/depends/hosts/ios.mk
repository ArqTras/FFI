# iOS device cross-compile (aarch64-apple-ios) for contrib/depends.
IOS_MIN_VERSION=13.0
IOS_SDK=$(shell xcrun --sdk iphoneos --show-sdk-path)

ios_CC=$(shell xcrun --sdk iphoneos --find clang) -target arm64-apple-ios$(IOS_MIN_VERSION) -isysroot$(IOS_SDK)
ios_CXX=$(shell xcrun --sdk iphoneos --find clang++) -target arm64-apple-ios$(IOS_MIN_VERSION) -isysroot$(IOS_SDK)
ios_AR=$(shell xcrun --sdk iphoneos --find ar)
ios_RANLIB=$(shell xcrun --sdk iphoneos --find ranlib)
ios_STRIP=$(shell xcrun --sdk iphoneos --find strip)
ios_NM=$(shell xcrun --sdk iphoneos --find nm)
ios_LIBTOOL=$(shell xcrun --sdk iphoneos --find libtool)
ios_INSTALL_NAME_TOOL=$(shell xcrun --sdk iphoneos --find install_name_tool)
ios_OTOOL=$(shell xcrun --sdk iphoneos --find otool)

ios_CFLAGS=-pipe -fno-stack-check
ios_CXXFLAGS=$(ios_CFLAGS)
ios_CPPFLAGS=
ios_LDFLAGS=-target arm64-apple-ios$(IOS_MIN_VERSION) -isysroot$(IOS_SDK)
ios_ARFLAGS=crs

ios_release_CFLAGS=-O2
ios_release_CXXFLAGS=$(ios_release_CFLAGS)

ios_debug_CFLAGS=-O1
ios_debug_CXXFLAGS=$(ios_debug_CFLAGS)

ios_cmake_system=iOS

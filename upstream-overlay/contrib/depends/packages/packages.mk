packages:=boost openssl sodium zeromq unbound

# No USB HID / libusb on iOS (IOKit HID headers unavailable; wallet FFI does not link hidapi).
ifneq ($(host_os),android)
ifneq ($(host_os),ios)
packages += libusb hidapi
endif
endif

ifneq ($(host_os),mingw32)
ifneq ($(host_os),ios)
packages += ncurses readline
endif
endif

linux_native_packages:=
linux_packages:=

ifeq ($(build_tests),ON)
packages += gtest
endif

ifneq ($(build_os), darwin)
darwin_native_packages := darwin_sdk native_cctools native_libtapi
endif
darwin_packages := ncurses readline

android_native_packages := android_ndk
android_packages := ncurses readline

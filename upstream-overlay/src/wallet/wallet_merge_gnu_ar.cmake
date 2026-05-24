# Merges dependency static archives into libwallet_merged using GNU ar MRI syntax.
# Invoked: cmake -DCMAKE_AR=ar -DWALLET_MERGED=... -DLIBEPEE=... -DLIBEASYLOGGING=... -DLIBRANDOMX=... -P wallet_merge_gnu_ar.cmake

foreach (_v CMAKE_AR WALLET_MERGED LIBEPEE LIBEASYLOGGING LIBRANDOMX)
  if(NOT DEFINED ${_v} OR "${${_v}}" STREQUAL "")
    message(FATAL_ERROR "wallet_merge_gnu_ar: ${_v} not set")
  endif()
endforeach()

string(REPLACE "\\" "/" WALLET_MERGED "${WALLET_MERGED}")
string(REPLACE "\\" "/" LIBEPEE "${LIBEPEE}")
string(REPLACE "\\" "/" LIBEASYLOGGING "${LIBEASYLOGGING}")
string(REPLACE "\\" "/" LIBRANDOMX "${LIBRANDOMX}")

set(_mri "OPEN ${WALLET_MERGED}
ADDLIB ${LIBEPEE}
ADDLIB ${LIBEASYLOGGING}
ADDLIB ${LIBRANDOMX}
SAVE
END
")

get_filename_component(_mdb "${WALLET_MERGED}" DIRECTORY)
set(_mri_file "${_mdb}/wallet_merge_fat.mri")
file(WRITE "${_mri_file}" "${_mri}")

execute_process(
  COMMAND "${CMAKE_AR}" -M
  INPUT_FILE "${_mri_file}"
  RESULT_VARIABLE _rc
)

if(NOT _rc EQUAL 0)
  message(FATAL_ERROR "wallet_merge_gnu_ar: `${CMAKE_AR} -M` failed (${_rc}); merged archive was not produced.")
endif()

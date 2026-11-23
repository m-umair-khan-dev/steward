// The system clipboard for file lists (P2-T04). Only moves bytes; the
// formats are encoded and decoded in Rust (steward_core::clip).
#pragma once

#include "rust/cxx.h"

void clipboard_set(const rust::Vec<rust::String>& mimes, const rust::Vec<rust::String>& data);
rust::String clipboard_get(rust::Str mime);

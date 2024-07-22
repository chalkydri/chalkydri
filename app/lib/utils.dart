import 'dart:io';

enum Edition {
  ds,
  web,
}

/// Check if running on a desktop platform
bool isDS() {
  return Platform.isLinux || Platform.isWindows;
}

/// Get edition
Edition getEdition() {
  if (isDS()) {
    return Edition.ds;
  } else {
    return Edition.web;
  }
}

/// Get edition as a string
String getEditionString() {
  if (isDS()) {
    return 'DS';
  } else {
    return 'Web';
  }
}

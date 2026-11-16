'use strict';

// A minimal reader for the one archive shape the release produces: a single
// file at the archive root, compressed with stored (0) or deflate (8), no
// zip64. Anything this parser does not understand is rejected with a named
// reason instead of guessed at, and every read is bounded by the sizes the
// archive itself declares.
//
// Written against the PKWARE APPNOTE layout with the Node standard library
// only: no `unzip`, no `tar`, no PowerShell shell-out, no dependency.

const fs = require('node:fs');
const path = require('node:path');
const zlib = require('node:zlib');

const EOCD_SIGNATURE = 0x06054b50;
const CENTRAL_SIGNATURE = 0x02014b50;
const LOCAL_SIGNATURE = 0x04034b50;

const EOCD_SIZE = 22;
const MAX_COMMENT = 0xffff;
const CENTRAL_HEADER_SIZE = 46;
const LOCAL_HEADER_SIZE = 30;

// The real binary is ~600 KB, packed into a ~290 KB zip. 64 MiB is far above
// anything legitimate and far below "fill the heap".
const MAX_UNCOMPRESSED = 64 * 1024 * 1024;

/** Throws a parser error carrying the reason. */
function fail(reason) {
  throw new Error(`Unusable polacode-mcp archive: ${reason}`);
}

/** Finds the End of Central Directory record, scanning back over the comment. */
function endOfCentralDirectory(archive) {
  const last = archive.length - EOCD_SIZE;
  const first = Math.max(0, archive.length - EOCD_SIZE - MAX_COMMENT);
  for (let offset = last; offset >= first; offset -= 1) {
    if (archive.readUInt32LE(offset) === EOCD_SIGNATURE) {
      return offset;
    }
  }
  return fail(
    `no end-of-central-directory record in ${archive.length} bytes, so the file is truncated or not a zip`
  );
}

/**
 * Returns `{ name, data }` for the entry named exactly `expectedName`.
 *
 * Multi-entry archives are searched rather than refused, but `expectedName` is
 * the only entry ever used; a missing entry is an error and lists what the
 * archive did contain.
 */
function readEntry(archive, expectedName) {
  if (!Buffer.isBuffer(archive)) {
    archive = Buffer.from(archive);
  }

  const eocd = endOfCentralDirectory(archive);
  const totalEntries = archive.readUInt16LE(eocd + 10);
  const directorySize = archive.readUInt32LE(eocd + 12);
  const directoryOffset = archive.readUInt32LE(eocd + 16);

  if (totalEntries === 0xffff || directorySize === 0xffffffff || directoryOffset === 0xffffffff) {
    return fail('zip64 archives are not supported (the entry count or offset is the zip64 marker)');
  }
  if (totalEntries === 0) {
    return fail('the archive contains no entries');
  }
  if (directoryOffset + directorySize > archive.length) {
    return fail('the central directory runs past the end of the archive');
  }

  let offset = directoryOffset;
  let found = null;
  const names = [];

  for (let index = 0; index < totalEntries; index += 1) {
    if (offset + CENTRAL_HEADER_SIZE > archive.length) {
      return fail('a central directory entry header runs past the end of the archive');
    }
    if (archive.readUInt32LE(offset) !== CENTRAL_SIGNATURE) {
      return fail(`central directory entry ${index + 1} has no valid signature`);
    }

    const method = archive.readUInt16LE(offset + 10);
    const compressedSize = archive.readUInt32LE(offset + 20);
    const uncompressedSize = archive.readUInt32LE(offset + 24);
    const nameLength = archive.readUInt16LE(offset + 28);
    const extraLength = archive.readUInt16LE(offset + 30);
    const commentLength = archive.readUInt16LE(offset + 32);
    const localOffset = archive.readUInt32LE(offset + 42);
    const next = offset + CENTRAL_HEADER_SIZE + nameLength + extraLength + commentLength;

    if (next > archive.length) {
      return fail('a central directory entry runs past the end of the archive');
    }

    const nameStart = offset + CENTRAL_HEADER_SIZE;
    const name = archive.toString('utf8', nameStart, nameStart + nameLength);
    names.push(name);
    if (name === expectedName && found === null) {
      found = { name, method, compressedSize, uncompressedSize, localOffset };
    }
    offset = next;
  }

  if (found === null) {
    const listed = names.length === 0 ? 'no entries' : `entries: ${names.join(', ')}`;
    return fail(
      `it does not contain \`${expectedName}\` at its root (${listed}); ` +
        'a release zip holds exactly that one file'
    );
  }
  if (found.compressedSize > MAX_UNCOMPRESSED || found.uncompressedSize > MAX_UNCOMPRESSED) {
    return fail(
      `\`${found.name}\` declares ${Math.max(found.compressedSize, found.uncompressedSize)} ` +
        `bytes, more than the ${MAX_UNCOMPRESSED} byte limit`
    );
  }

  const local = found.localOffset;
  if (local + LOCAL_HEADER_SIZE > archive.length) {
    return fail('the local file header runs past the end of the archive');
  }
  if (archive.readUInt32LE(local) !== LOCAL_SIGNATURE) {
    return fail('the local file header has no valid signature');
  }

  const localNameLength = archive.readUInt16LE(local + 26);
  const localExtraLength = archive.readUInt16LE(local + 28);
  const dataStart = local + LOCAL_HEADER_SIZE + localNameLength + localExtraLength;
  const dataEnd = dataStart + found.compressedSize;
  // Entry data lives before the central directory, so that offset is the honest
  // end of the file data: a declared size reaching past it is a lie or a cut.
  if (dataEnd > directoryOffset) {
    return fail(
      `the data of \`${found.name}\` needs ${found.compressedSize} bytes but only ` +
        `${Math.max(0, directoryOffset - dataStart)} are available before the central directory`
    );
  }

  const raw = archive.subarray(dataStart, dataEnd);
  let data;

  if (found.method === 0) {
    if (found.compressedSize !== found.uncompressedSize) {
      return fail(
        `\`${found.name}\` is stored but declares ${found.compressedSize} compressed and ` +
          `${found.uncompressedSize} uncompressed bytes`
      );
    }
    data = Buffer.from(raw);
  } else if (found.method === 8) {
    try {
      data = zlib.inflateRawSync(raw, { maxOutputLength: Math.max(found.uncompressedSize, 1) });
    } catch (error) {
      return fail(`\`${found.name}\` could not be inflated: ${error.message}`);
    }
  } else {
    return fail(
      `\`${found.name}\` uses compression method ${found.method}; ` +
        'only stored (0) and deflate (8) are supported'
    );
  }

  if (data.length !== found.uncompressedSize) {
    return fail(
      `\`${found.name}\` unpacked to ${data.length} bytes but the archive declares ` +
        `${found.uncompressedSize}`
    );
  }

  return { name: found.name, data };
}

/**
 * Writes `data` to `target` through a temporary file in the same directory and
 * an atomic rename, so an interrupted write never leaves a truncated file at
 * the final path. Non-Windows gets mode 0o755.
 */
function writeAtomic(target, data) {
  const directory = path.dirname(target);
  fs.mkdirSync(directory, { recursive: true });

  const temporary = path.join(
    directory,
    `.${path.basename(target)}.${process.pid}.${Date.now().toString(36)}.part`
  );

  try {
    fs.writeFileSync(temporary, data, { mode: 0o755 });
    if (process.platform !== 'win32') {
      // writeFileSync's mode is masked by the umask; the chmod makes the
      // executable bits explicit.
      fs.chmodSync(temporary, 0o755);
    }
    fs.renameSync(temporary, target);
  } catch (error) {
    try {
      fs.rmSync(temporary, { force: true });
    } catch {
      // The temp file is already gone; the original error is the one to keep.
    }
    throw error;
  }
}

/** Extracts `expectedName` from the archive to `target`. Returns the byte count. */
function extractToFile(archive, expectedName, target) {
  const entry = readEntry(archive, expectedName);
  writeAtomic(target, entry.data);
  return entry.data.length;
}

module.exports = { MAX_UNCOMPRESSED, readEntry, extractToFile, writeAtomic };

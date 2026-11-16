'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const zlib = require('node:zlib');

const { MAX_UNCOMPRESSED, readEntry, extractToFile } = require('../lib/zip');

function temporaryDirectory() {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'polacode-zip-test-'));
}

function isWindows() {
  return process.platform === 'win32';
}

// ---------------------------------------------------------------------------
// Control 1: a complete zip spelled out byte by byte.
//
// This literal was written by hand from the PKWARE APPNOTE layout, not produced
// by anything in this package: if the installer's reader and the installer's
// writer agreed with each other only, that would prove nothing.
//
//   local header   50 4b 03 04 | version 20 | flags 0 | method 0 (stored)
//                  time 0 date 0 | crc32 0x363a3020 | csize 6 | usize 6
//                  name len 17 | extra len 0 | "zpolacode-mcp.exe" | "hello\n"
//   central entry  50 4b 01 02 | ... same fields + local offset 0
//   EOCD           50 4b 05 06 | entries 1 | dir size 63 | dir offset 53
//
// Total: 53 + 63 + 22 = 138 bytes.
const STORED_ARCHIVE = Buffer.from([
  // local file header (30)
  0x50, 0x4b, 0x03, 0x04, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
  0x20, 0x30, 0x3a, 0x36, 0x06, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x11, 0x00,
  0x00, 0x00,
  // name "zpolacode-mcp.exe" (17)
  0x7a, 0x70, 0x6f, 0x6c, 0x61, 0x63, 0x6f, 0x64, 0x65, 0x2d, 0x6d, 0x63, 0x70, 0x2e,
  0x65, 0x78, 0x65,
  // data "hello\n" (6)
  0x68, 0x65, 0x6c, 0x6c, 0x6f, 0x0a,
  // central directory entry (46 + 17)
  0x50, 0x4b, 0x01, 0x02, 0x14, 0x00, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
  0x00, 0x00, 0x20, 0x30, 0x3a, 0x36, 0x06, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00,
  0x11, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
  0x00, 0x00, 0x00, 0x00,
  0x7a, 0x70, 0x6f, 0x6c, 0x61, 0x63, 0x6f, 0x64, 0x65, 0x2d, 0x6d, 0x63, 0x70, 0x2e,
  0x65, 0x78, 0x65,
  // end of central directory (22)
  0x50, 0x4b, 0x05, 0x06, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x3f, 0x00,
  0x00, 0x00, 0x35, 0x00, 0x00, 0x00, 0x00, 0x00,
]);

test('the hand-written stored archive is 138 bytes and is what this reader expects', () => {
  assert.equal(STORED_ARCHIVE.length, 138);
  assert.equal(readEntry(STORED_ARCHIVE, 'zpolacode-mcp.exe').data.toString(), 'hello\n');
});

test('extraction writes the stored entry and makes it executable', () => {
  const directory = temporaryDirectory();
  const target = path.join(directory, 'zpolacode-mcp.exe');

  const written = extractToFile(STORED_ARCHIVE, 'zpolacode-mcp.exe', target);

  assert.equal(written, 6);
  assert.equal(fs.readFileSync(target, 'utf8'), 'hello\n');
  assert.equal(fs.statSync(target).size, 6);
  if (!isWindows()) {
    assert.equal(fs.statSync(target).mode & 0o111, 0o111, 'mode 0o755 expected');
  }
});

// ---------------------------------------------------------------------------
// Control 2: method 8 (deflate). The deflate stream comes from zlib, which is
// the decoder's counterpart, but the local header, central directory, sizes and
// CRC are assembled here by the test. Nothing here calls the installer.
function u16(value) {
  const bytes = Buffer.alloc(2);
  bytes.writeUInt16LE(value);
  return bytes;
}

function u32(value) {
  const bytes = Buffer.alloc(4);
  bytes.writeUInt32LE(value >>> 0);
  return bytes;
}

const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let index = 0; index < 256; index += 1) {
    let value = index;
    for (let bit = 0; bit < 8; bit += 1) {
      value = value & 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
    }
    table[index] = value >>> 0;
  }
  return table;
})();

function crc32(buffer) {
  let crc = 0xffffffff;
  for (const byte of buffer) {
    crc = CRC_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

/**
 * Assembles a single-entry archive from explicit fields. Test-only: this is the
 * control, never the code under test.
 */
function buildArchive({
  name,
  data,
  method = 0,
  compressedSize = data.length,
  uncompressedSize = data.length,
  crcOf = data,
  entries = 1,
}) {
  const nameBytes = Buffer.from(name, 'utf8');
  const crc = crc32(crcOf);

  const local = Buffer.concat([
    Buffer.from([0x50, 0x4b, 0x03, 0x04]),
    u16(20),
    u16(0),
    u16(method),
    u16(0),
    u16(0),
    u32(crc),
    u32(compressedSize),
    u32(uncompressedSize),
    u16(nameBytes.length),
    u16(0),
    nameBytes,
    data,
  ]);

  const central = Buffer.concat([
    Buffer.from([0x50, 0x4b, 0x01, 0x02]),
    u16(20),
    u16(20),
    u16(0),
    u16(method),
    u16(0),
    u16(0),
    u32(crc),
    u32(compressedSize),
    u32(uncompressedSize),
    u16(nameBytes.length),
    u16(0),
    u16(0),
    u16(0),
    u16(0),
    u32(0),
    u32(0),
    nameBytes,
  ]);

  const eocd = Buffer.concat([
    Buffer.from([0x50, 0x4b, 0x05, 0x06]),
    u16(0),
    u16(0),
    u16(entries),
    u16(entries),
    u32(central.length),
    u32(local.length),
    u16(0),
  ]);

  return Buffer.concat([local, central, eocd]);
}

test('extraction inflates a deflated entry whose header the test assembled', () => {
  const content = Buffer.from('zpolacode-mcp 0.2.1\n');
  const archive = buildArchive({
    name: 'zpolacode-mcp',
    data: zlib.deflateRawSync(content),
    method: 8,
    uncompressedSize: content.length,
    crcOf: content,
  });

  const directory = temporaryDirectory();
  const target = path.join(directory, 'zpolacode-mcp');
  extractToFile(archive, 'zpolacode-mcp', target);

  assert.equal(fs.readFileSync(target, 'utf8'), content.toString());
  if (!isWindows()) {
    assert.equal(fs.statSync(target).mode & 0o111, 0o111);
  }
});

test('an unsupported compression method is rejected by number', () => {
  const archive = buildArchive({ name: 'zpolacode-mcp.exe', data: Buffer.from('x'), method: 12 });
  assert.throws(
    () => readEntry(archive, 'zpolacode-mcp.exe'),
    /compression method 12.*only stored \(0\) and deflate \(8\)/
  );
});

test('an archive without the expected entry lists what it did contain', () => {
  const archive = buildArchive({ name: 'README.txt', data: Buffer.from('not the binary') });
  assert.throws(
    () => readEntry(archive, 'zpolacode-mcp.exe'),
    (error) => {
      assert.match(error.message, /does not contain `zpolacode-mcp\.exe`/);
      assert.match(error.message, /entries: README\.txt/);
      return true;
    }
  );
});

test('a truncated archive is rejected, not partially read', () => {
  const cutTail = STORED_ARCHIVE.subarray(0, STORED_ARCHIVE.length - 30);
  assert.throws(() => readEntry(cutTail, 'zpolacode-mcp.exe'), /truncated or not a zip/);

  // EOCD intact, entry data cut short: the declared compressed size no longer
  // fits between the local header and the end of the file.
  const shortData = buildArchive({
    name: 'zpolacode-mcp.exe',
    data: Buffer.from('hello\n'),
    compressedSize: 6 + 32,
    uncompressedSize: 6 + 32,
  });
  assert.throws(
    () => readEntry(shortData, 'zpolacode-mcp.exe'),
    /needs 38 bytes but only 6 are available before the central directory/
  );
});

test('an oversized declared size is rejected before anything is inflated', () => {
  const archive = buildArchive({
    name: 'zpolacode-mcp.exe',
    data: Buffer.from('hello\n'),
    compressedSize: 6,
    uncompressedSize: MAX_UNCOMPRESSED + 1,
  });
  assert.throws(
    () => readEntry(archive, 'zpolacode-mcp.exe'),
    new RegExp(`more than the ${MAX_UNCOMPRESSED} byte limit`)
  );
});

test('zip64 archives are rejected instead of misread', () => {
  const archive = buildArchive({ name: 'zpolacode-mcp.exe', data: Buffer.from('x'), entries: 0xffff });
  assert.throws(() => readEntry(archive, 'zpolacode-mcp.exe'), /zip64 archives are not supported/);
});

test('the atomic write leaves no temporary file behind', () => {
  const directory = temporaryDirectory();
  const target = path.join(directory, 'nested', 'zpolacode-mcp.exe');
  extractToFile(STORED_ARCHIVE, 'zpolacode-mcp.exe', target);

  const leftovers = fs
    .readdirSync(path.dirname(target))
    .filter((entry) => entry !== 'zpolacode-mcp.exe');
  assert.deepEqual(leftovers, []);
});

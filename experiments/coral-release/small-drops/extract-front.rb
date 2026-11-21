# Extract the original CImg packer's front signed-distance byte from each RG pair.
require 'zlib'

abort 'Usage: ruby extract-front.rb INPUT.rg8.gz NEW-FRONT.r8.gz' unless ARGV.size == 2
geometry = [704, 704, 96, 24, 96, 208, 512, 288]
width, height, frames = geometry
Zlib::GzipReader.open(ARGV[0]) do |source|
  expected = 'TSSDF002' + geometry.pack('V*')
  abort 'Unexpected world geometry' unless source.read(expected.bytesize) == expected
  File.open(ARGV[1], 'wbx') do |out|
    Zlib::GzipWriter.wrap(out, 6) do |front|
      front.mtime = 0
      frames.times do
        pairs = source.read(2 * width * height)
        abort 'Truncated source frame' unless pairs && pairs.bytesize == 2 * width * height
        # Each unsigned big-endian 16-bit pair has the green channel in its low byte.
        front.write(pairs.unpack('n*').pack('C*'))
      end
      abort 'Trailing cache data' unless source.read(1).nil?
    end
  end
end

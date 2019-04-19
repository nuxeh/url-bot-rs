with import <nixpkgs> { };

rustPlatform.buildRustPackage rec {
  name = "url-bot-rs-${version}";
  version = "0.2.0";
  src = ./.;
  buildInputs = [ openssl pkgconfig sqlite ];

  checkPhase = "";

  cargoSha256 = "05sw69h81773lw85bqr9xnrxbiw2z8hk5y6ck9vfp2r4dr5i43d9";

  meta = with stdenv.lib; {
    description = "Minimal IRC URL bot in Rust";
    homepage = https://github.com/nuxeh/url-bot-rs;
    license = licenses.isc;
    maintainers = [ maintainers.tailhook ];
    platforms = platforms.all;
  };
}

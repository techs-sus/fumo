{
  lib,
  rustPlatform,
  openssl,
  pkg-config,
  ...
}:
rustPlatform.buildRustPackage {
  pname = "fumo";
  version = "0.1.0";

  src = ./.;

  cargoLock = {
    lockFile = ./Cargo.lock;
    allowBuiltinFetchGit = true;
  };

  buildFeatures = []; # cargo build features

  nativeBuildInputs = [
    pkg-config
  ];

  buildInputs = [
    openssl
  ];

  meta = {
    description = "fumo is a cli tool built to interact with the fumosclub API.";
    homepage = "https://github.com/techs-sus/fumo";
    license = lib.licenses.mit;
    maintainers = [
      {
        name = "techs-sus";
        github = "techs-sus";
        githubId = 92276908;
      }
    ];
    platforms = lib.platforms.unix;
    mainProgram = "fumo";
  };
}

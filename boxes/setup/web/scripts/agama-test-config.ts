// Konfiguracja Agamy dla testu instalacji w QEMU (tools/qemu-install-test.py): ta sama funkcja,
// której używa Setup, dla pustego dysku /dev/vda i konta testowego.
//   npx tsx scripts/agama-test-config.ts > config.json
import { installConfig } from "../src/backend/agama";

const plan = {
  locale: "pl_PL.UTF-8",
  timezone: "Europe/Warsaw",
  keymap: "pl",
  disk: { device: process.argv[2] || "/dev/vda", name: "QEMU", sizeGB: 40 },
  place: "whole" as const,
  fullName: "Jan Testowy",
  userName: "jan",
  password: "shrimp",
};
process.stdout.write(JSON.stringify(installConfig(plan), null, 1) + "\n");

import { first, second } from "./producer";

declare const enabled: boolean;
export const selected = enabled ? first : second;

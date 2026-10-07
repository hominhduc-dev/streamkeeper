import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwind from '@tailwindcss/vite';
export default defineConfig({plugins:[react(),tailwind()],server:{port:1420,strictPort:true},clearScreen:false,envPrefix:['VITE_','TAURI_ENV_'],build:{target:'chrome105'}});

import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';

// https://vite.dev/config/
export default defineConfig({
    plugins: [react()],
    root: path.join(__dirname, 'src'),
    resolve: {
        alias: {
            '@': path.resolve(__dirname, 'src'),
        },
    },
    base: './',
    build: {
        outDir: '../dist',
        emptyOutDir: true,
    },
    server: {
        port: 3000,
    },
});

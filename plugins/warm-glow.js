globalThis.formaPlugin = {
    id: "warm-glow",
    name: "Warm Glow",
    version: "1.0.0",
    apply(image) {
        const data = image.data;
        for (let i = 0; i < data.length; i += 4) {
            data[i] = Math.min(255, data[i] + 14);
            data[i + 1] = Math.min(255, data[i + 1] + 5);
            data[i + 2] = Math.max(0, data[i + 2] - 8);
        }
        return image;
    }
};

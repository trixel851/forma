globalThis.formaPlugin = {
    id: "soft-matte",
    name: "Soft Matte",
    version: "1.0.0",
    apply(image) {
        const data = image.data;
        for (let i = 0; i < data.length; i += 4) {
            for (let channel = 0; channel < 3; channel += 1) {
                const value = data[i + channel] / 255;
                const soft = value * value * (3 - 2 * value);
                data[i + channel] = Math.round(soft * 210 + 22);
            }
        }
        return image;
    }
};

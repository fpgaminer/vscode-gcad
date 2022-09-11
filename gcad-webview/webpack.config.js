const path = require('path');
const WasmPackPlugin = require('@wasm-tool/wasm-pack-plugin');
const HtmlWebpackPlugin = require('html-webpack-plugin');

module.exports = {
	mode: 'development',
	entry: './test.index.js',
	devtool: 'inline-source-map',

	devServer: {
		static: './dist',
	},
	plugins: [
		new WasmPackPlugin({
			crateDirectory: path.resolve(__dirname, "."),
		}),
		new HtmlWebpackPlugin({
			template: 'index.html',
			title: 'Development',
		}),
	],

	output: {
		filename: '[name].bundle.js',
		path: path.resolve(__dirname, 'dist'),
		clean: true,
	},

	optimization: {
		runtimeChunk: 'single',
	},

	experiments: {
		asyncWebAssembly: true
	}
};
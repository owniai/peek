import React from "react";

interface Props {
  title: string;
  children: React.ReactNode;
}

function FunctionalComponent(props: Props): JSX.Element {
  return <div>{props.title}</div>;
}

const ArrowComponent = ({ title }: Props) => <div>{title}</div>;

class ClassComponent extends React.Component<Props> {
  render(): JSX.Element {
    return <div>{this.props.title}</div>;
  }
}

function GenericList<T>({ items }: { items: T[] }): JSX.Element {
  return <ul>{items.map((item, i) => <li key={i}>{String(item)}</li>)}</ul>;
}

export default function Page(): JSX.Element {
  return <main><FunctionalComponent title="Hello" /></main>;
}

export { FunctionalComponent, ArrowComponent };
